use std::path::PathBuf;

use anyhow::Result;

use crate::db::{Database, RowPreview};

use super::{Action, App, AppMode, ContentView, PaneFocus, SqlPane, SqlResultState, SqlState};

impl App {
    pub fn load(path: PathBuf) -> Result<Self> {
        let db = Database::open(&path)?;
        let tables = db.list_tables()?;
        let mut app = Self {
            path,
            mode: AppMode::Browse,
            db,
            tables,
            selected_table: 0,
            focus: PaneFocus::Tables,
            content_view: ContentView::Rows,
            row_offset: 0,
            row_limit: super::DEFAULT_ROW_LIMIT,
            selected_row: 0,
            schema_offset: 0,
            schema_page_lines: super::DEFAULT_SCHEMA_PAGE_LINES,
            preview: RowPreview::empty(),
            details: None,
            detail: None,
            filter_modal: None,
            modal: None,
            search: None,
            sql: SqlState {
                query: String::new(),
                cursor: 0,
                editor_scroll: 0,
                editor_height: 8,
                focus: SqlPane::Editor,
                history: Vec::new(),
                history_offset: 0,
                history_height: 8,
                selected_history: 0,
                result: SqlResultState::Empty,
                result_scroll: 0,
                result_height: 8,
                completion: None,
                status: "SQL mode ready".to_string(),
            },
            configs: std::collections::HashMap::new(),
        };
        app.refresh_preview()?;
        Ok(app)
    }

    pub fn handle(&mut self, action: Action) -> Result<()> {
        if matches!(action, Action::SwitchToBrowse) {
            self.sql.completion = None;
            self.mode = AppMode::Browse;
            return Ok(());
        }
        if matches!(action, Action::SwitchToSql) {
            self.mode = AppMode::Sql;
            self.detail = None;
            self.modal = None;
            self.filter_modal = None;
            self.search = None;
            return Ok(());
        }

        if self.mode == AppMode::Sql {
            return self.handle_sql(action);
        }

        if self.detail.is_some() {
            return self.handle_detail(action);
        }

        if self.filter_modal.is_some() {
            return self.handle_filter_modal(action);
        }

        if self.modal.is_some() {
            return self.handle_modal(action);
        }

        if self.search.is_some() {
            return self.handle_search(action);
        }

        match action {
            Action::None => {}
            Action::Quit => {}
            Action::ToggleFocus | Action::ReverseFocus => self.toggle_focus(),
            Action::ToggleView => self.toggle_view(),
            Action::MoveUp => self.move_up()?,
            Action::MoveDown => self.move_down()?,
            Action::MoveLeft | Action::MoveRight => self.toggle_focus(),
            Action::OpenConfig => self.open_config_modal(),
            Action::OpenFilters => self.open_filter_modal(),
            Action::Reload => self.reload()?,
            Action::OpenSearchCurrent => self.open_search(super::SearchScope::CurrentTable)?,
            Action::OpenSearchAll => self.open_search(super::SearchScope::AllTables)?,
            Action::Confirm => self.open_detail()?,
            Action::CloseModal
            | Action::ToggleItem
            | Action::FollowLink
            | Action::Delete
            | Action::Clear
            | Action::MoveHome
            | Action::MoveEnd
            | Action::PageUp
            | Action::PageDown
            | Action::ExecuteSql
            | Action::NewLine
            | Action::InputChar(_)
            | Action::Backspace
            | Action::SwitchToBrowse
            | Action::SwitchToSql => {}
        }

        Ok(())
    }

    pub fn set_viewport_sizes(
        &mut self,
        row_limit: usize,
        schema_page_lines: usize,
        detail_value_width: usize,
        detail_value_height: usize,
    ) -> Result<()> {
        let row_limit = row_limit.max(1);
        let schema_page_lines = schema_page_lines.max(1);
        let detail_value_width = detail_value_width.max(1);
        let detail_value_height = detail_value_height.max(1);
        let mut needs_refresh = false;

        if self.row_limit != row_limit {
            self.row_limit = row_limit;
            self.clamp_row_viewport();
            needs_refresh = true;
        }

        if self.schema_page_lines != schema_page_lines {
            self.schema_page_lines = schema_page_lines;
            self.clamp_schema_offset();
        }

        if let Some(detail) = &mut self.detail {
            detail.value_view_width = detail_value_width;
            detail.value_view_height = detail_value_height;
            self.clamp_detail_scroll();
        }

        if let Some(search) = &mut self.search {
            search.result_limit = row_limit.saturating_sub(3).max(1);
            self.clamp_search_viewport();
        }

        self.ensure_sql_viewport();

        if needs_refresh {
            self.refresh_preview()?;
        }

        Ok(())
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    pub fn selected_table_name(&self) -> Option<&str> {
        self.tables
            .get(self.selected_table)
            .map(|table| table.name.as_str())
    }

    pub fn display_table_name(&self, table_name: &str) -> String {
        match split_table_name(table_name) {
            Some(("main", bare_name)) if !self.has_table_name_collision(bare_name) => {
                bare_name.to_string()
            }
            _ => table_name.to_string(),
        }
    }

    pub fn selected_table_label(&self) -> Option<String> {
        self.selected_table_name()
            .map(|table_name| self.display_table_name(table_name))
    }

    pub fn selected_row_in_view(&self) -> Option<usize> {
        self.selected_row
            .checked_sub(self.row_offset)
            .and_then(|row| (row < self.preview.rows.len()).then_some(row))
    }

    pub fn table_pane_width(&self) -> u16 {
        let longest_name = self
            .tables
            .iter()
            .map(|table| self.display_table_name(&table.name).chars().count())
            .max()
            .unwrap_or("No tables".len());
        let width = longest_name.saturating_add(6);
        width.min(40) as u16
    }

    fn has_table_name_collision(&self, bare_name: &str) -> bool {
        self.tables
            .iter()
            .filter(|table| {
                split_table_name(&table.name)
                    .map(|(_, name)| name)
                    .unwrap_or(table.name.as_str())
                    == bare_name
            })
            .take(2)
            .count()
            > 1
    }

    pub(super) fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            PaneFocus::Tables => PaneFocus::Content,
            PaneFocus::Content => PaneFocus::Tables,
        };
    }

    pub(super) fn toggle_view(&mut self) {
        self.detail = None;
        self.content_view = match self.content_view {
            ContentView::Rows => ContentView::Schema,
            ContentView::Schema => ContentView::Rows,
        };
    }

    pub(super) fn move_up(&mut self) -> Result<()> {
        match self.focus {
            PaneFocus::Tables => self.move_table_selection_up()?,
            PaneFocus::Content => match self.content_view {
                ContentView::Rows => self.move_row_selection_up()?,
                ContentView::Schema => self.scroll_schema_up(),
            },
        }
        Ok(())
    }

    pub(super) fn move_down(&mut self) -> Result<()> {
        match self.focus {
            PaneFocus::Tables => self.move_table_selection_down()?,
            PaneFocus::Content => match self.content_view {
                ContentView::Rows => self.move_row_selection_down()?,
                ContentView::Schema => self.scroll_schema_down(),
            },
        }
        Ok(())
    }

    pub(super) fn reload(&mut self) -> Result<()> {
        self.refresh_loaded_db_state()
    }

    pub(super) fn refresh_loaded_db_state(&mut self) -> Result<()> {
        let selected_table_name = self.selected_table_name().map(str::to_owned);
        let selected_table_index = self.selected_table;
        self.tables = self.db.list_tables()?;
        self.selected_table = selected_table_name
            .as_deref()
            .and_then(|table_name| {
                self.tables
                    .iter()
                    .position(|table| table.name == table_name)
            })
            .unwrap_or_else(|| selected_table_index.min(self.tables.len().saturating_sub(1)));
        self.detail = None;
        self.reset_content_position();
        self.refresh_preview()?;
        Ok(())
    }

    pub fn request_quit(&mut self) -> Result<bool> {
        if self.detail.is_some()
            || self.filter_modal.is_some()
            || self.modal.is_some()
            || self.search.is_some()
            || self.sql.completion.is_some()
        {
            self.handle(Action::CloseModal)?;
            return Ok(false);
        }
        Ok(true)
    }

    pub(super) fn move_table_selection_up(&mut self) -> Result<()> {
        if self.selected_table > 0 {
            self.selected_table -= 1;
            self.detail = None;
            self.reset_content_position();
            self.refresh_preview()?;
        }
        Ok(())
    }

    pub(super) fn move_table_selection_down(&mut self) -> Result<()> {
        if self.selected_table + 1 < self.tables.len() {
            self.selected_table += 1;
            self.detail = None;
            self.reset_content_position();
            self.refresh_preview()?;
        }
        Ok(())
    }

    pub(super) fn move_row_selection_up(&mut self) -> Result<()> {
        if self.selected_row > 0 {
            self.detail = None;
            self.selected_row -= 1;
            let previous_offset = self.row_offset;
            self.clamp_row_viewport();
            if previous_offset != self.row_offset {
                self.refresh_preview()?;
            }
        }
        Ok(())
    }

    pub(super) fn move_row_selection_down(&mut self) -> Result<()> {
        if self.selected_row + 1 < self.preview.total_rows {
            self.detail = None;
            self.selected_row += 1;
            let previous_offset = self.row_offset;
            self.clamp_row_viewport();
            if previous_offset != self.row_offset {
                self.refresh_preview()?;
            }
        }
        Ok(())
    }

    pub(super) fn scroll_schema_up(&mut self) {
        if self.schema_offset > 0 {
            self.schema_offset -= 1;
        }
    }

    pub(super) fn scroll_schema_down(&mut self) {
        let max_offset = self.max_schema_offset();
        if self.schema_offset < max_offset {
            self.schema_offset += 1;
        }
    }

    pub(super) fn refresh_preview(&mut self) -> Result<()> {
        if let Some(table_name) = self.selected_table_name().map(str::to_owned) {
            self.details = Some(self.db.table_details(&table_name)?);
            self.ensure_table_config();

            if let Some(details) = &self.details {
                if details.total_rows == 0 {
                    self.selected_row = 0;
                    self.row_offset = 0;
                } else {
                    self.selected_row = self.selected_row.min(details.total_rows.saturating_sub(1));
                    self.clamp_row_viewport();
                }
            }

            self.preview = self.db.preview_table(
                &table_name,
                &self.visible_column_names(),
                &self.current_sort_clauses(),
                &self.current_filter_clauses(),
                self.row_limit,
                self.row_offset,
            )?;
            self.clamp_schema_offset();
        } else {
            self.details = None;
            self.preview = RowPreview::empty();
            self.selected_row = 0;
            self.row_offset = 0;
            self.schema_offset = 0;
            self.modal = None;
            self.search = None;
            self.detail = None;
        }

        Ok(())
    }

    pub(super) fn clamp_row_viewport(&mut self) {
        let total_rows = self
            .details
            .as_ref()
            .map(|details| details.total_rows)
            .unwrap_or(0);
        if total_rows == 0 {
            self.selected_row = 0;
            self.row_offset = 0;
            return;
        }

        self.selected_row = self.selected_row.min(total_rows.saturating_sub(1));
        let max_offset = total_rows.saturating_sub(self.row_limit);
        self.row_offset = self.row_offset.min(max_offset);

        if self.selected_row < self.row_offset {
            self.row_offset = self.selected_row;
        }
        if self.selected_row >= self.row_offset + self.row_limit {
            self.row_offset = self.selected_row + 1 - self.row_limit;
        }
    }

    pub(super) fn clamp_schema_offset(&mut self) {
        self.schema_offset = self.schema_offset.min(self.max_schema_offset());
    }

    fn max_schema_offset(&self) -> usize {
        self.schema_lines()
            .len()
            .saturating_sub(self.schema_page_lines)
    }

    pub(super) fn reset_content_position(&mut self) {
        self.selected_row = 0;
        self.row_offset = 0;
        self.schema_offset = 0;
    }
}

fn split_table_name(table_name: &str) -> Option<(&str, &str)> {
    table_name.split_once('.')
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use rusqlite::Connection;

    use super::App;

    #[test]
    fn refresh_loaded_db_state_preserves_selected_table_name() {
        let path = temp_db_path("refresh-selection");
        let conn = Connection::open(&path).expect("create db");
        conn.execute("CREATE TABLE users(id INTEGER PRIMARY KEY)", [])
            .expect("create users");
        drop(conn);

        let mut app = App::load(path.clone()).expect("load app");
        assert_eq!(app.selected_table_name(), Some("main.users"));

        app.db
            .execute_sql("CREATE TABLE addresses(id INTEGER PRIMARY KEY)", 10)
            .expect("create addresses");
        app.refresh_loaded_db_state().expect("refresh app state");

        assert_eq!(app.selected_table_name(), Some("main.users"));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn request_quit_closes_search_before_exiting() {
        let path = temp_db_path("request-quit-search");
        let conn = Connection::open(&path).expect("create db");
        conn.execute("CREATE TABLE users(id INTEGER PRIMARY KEY)", [])
            .expect("create users");
        drop(conn);

        let mut app = App::load(path.clone()).expect("load app");
        app.open_search(crate::app::SearchScope::CurrentTable)
            .expect("open search");

        let should_quit = app.request_quit().expect("request quit");

        assert!(!should_quit);
        assert!(app.search.is_none());

        let _ = fs::remove_file(path);
    }

    #[test]
    fn switching_to_browse_clears_sql_completion() {
        let path = temp_db_path("switch-browse-clears-completion");
        let conn = Connection::open(&path).expect("create db");
        conn.execute("CREATE TABLE users(id INTEGER PRIMARY KEY)", [])
            .expect("create users");
        drop(conn);

        let mut app = App::load(path.clone()).expect("load app");
        app.mode = crate::app::AppMode::Sql;
        app.sql.completion = Some(crate::app::SqlCompletionState {
            prefix_start: 0,
            items: vec![crate::app::SqlCompletionItem {
                label: "SELECT".to_string(),
                insert_text: "SELECT".to_string(),
            }],
            selected: 0,
        });

        app.handle(crate::app::Action::SwitchToBrowse)
            .expect("switch to browse");

        assert_eq!(app.mode, crate::app::AppMode::Browse);
        assert!(app.sql.completion.is_none());

        let _ = fs::remove_file(path);
    }

    #[test]
    fn reload_preserves_connection_scoped_tables() {
        let path = temp_db_path("reload-preserves-temp");
        let conn = Connection::open(&path).expect("create db");
        conn.execute("CREATE TABLE users(id INTEGER PRIMARY KEY)", [])
            .expect("create users");
        drop(conn);

        let mut app = App::load(path.clone()).expect("load app");
        app.db
            .execute_sql("CREATE TEMP TABLE scratch(value TEXT)", 10)
            .expect("create temp table");

        app.reload().expect("reload");

        assert!(
            app.tables.iter().any(|table| table.name == "temp.scratch"),
            "reload should keep connection-scoped temp tables visible"
        );

        let _ = fs::remove_file(path);
    }

    fn temp_db_path(label: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("squid-core-{label}-{stamp}.sqlite"))
    }
}
