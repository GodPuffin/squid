use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};

use crate::db::{Database, RowPreview};

use super::{
    Action, App, AppMode, ContentView, FilterRule, PaneFocus, RecentStore, SortRule, SqlPane,
    SqlResultState, SqlState,
    home::{
        AppStorage, StoredFilterRule, StoredSession, StoredSortRule, StoredTableState,
        normalize_database_path,
    },
};

impl App {
    pub fn load(path: impl Into<Option<PathBuf>>) -> Result<Self> {
        let path = path.into();
        let (recent_items, status_message) = match RecentStore::load() {
            Ok(items) => (items, None),
            Err(error) => (Vec::new(), Some(format!("Could not load recents: {error}"))),
        };

        let mut app = Self {
            mode: AppMode::Home,
            path: None,
            db: None,
            tables: Vec::new(),
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
            search_results_view_width: 0,
            recent_items,
            selected_recent: 0,
            status_message,
            sql: SqlState {
                query: String::new(),
                cursor: 0,
                editor_scroll: 0,
                editor_col_offset: 0,
                editor_height: 8,
                editor_width: 40,
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
                column_cache: std::collections::HashMap::new(),
                completion_cache_query: String::new(),
                completion_candidates_cache: std::collections::HashMap::new(),
            },
            configs: std::collections::HashMap::new(),
        };

        if let Some(path) = path.filter(|path| super::home::recent_path_is_available(path)) {
            if let Err(error) = app.open_database(&path) {
                app.status_message = Some(format!("Could not restore {}: {error}", path.display()));
                app.refresh_home_selection();
            }
        } else {
            app.refresh_home_selection();
        }

        Ok(app)
    }

    pub fn handle(&mut self, action: Action) -> Result<()> {
        if self.is_home() {
            return self.handle_home(action);
        }

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
            | Action::EditDetail
            | Action::SaveDetail
            | Action::DiscardDetail
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
        if self.is_home() {
            return Ok(());
        }

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

    pub fn path(&self) -> Option<&PathBuf> {
        self.path.as_ref()
    }

    pub fn is_home(&self) -> bool {
        self.mode == AppMode::Home
    }

    pub fn selected_recent_item(&self) -> Option<&super::RecentItem> {
        self.recent_items.get(self.selected_recent)
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
        if self.is_home() {
            let longest_path = self
                .recent_items
                .iter()
                .map(|item| item.path.display().to_string().chars().count())
                .max()
                .unwrap_or("No recent files".len());
            let width = longest_path.saturating_add(6);
            return width.min(48) as u16;
        }

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
        if self.is_home() {
            return;
        }
        self.detail = None;
        self.content_view = match self.content_view {
            ContentView::Rows => ContentView::Schema,
            ContentView::Schema => ContentView::Rows,
        };
    }

    pub(super) fn move_up(&mut self) -> Result<()> {
        if self.is_home() {
            if self.focus == PaneFocus::Tables {
                self.move_recent_selection_up();
            }
            return Ok(());
        }

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
        if self.is_home() {
            if self.focus == PaneFocus::Tables {
                self.move_recent_selection_down();
            }
            return Ok(());
        }

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
        if self.is_home() {
            match RecentStore::load() {
                Ok(items) => {
                    self.recent_items = items;
                    self.refresh_home_selection();
                    self.status_message = Some("Reloaded recent databases".to_string());
                }
                Err(error) => {
                    self.status_message = Some(format!("Could not reload recents: {error}"));
                }
            }
            return Ok(());
        }

        self.refresh_loaded_db_state()
    }

    pub(super) fn refresh_loaded_db_state(&mut self) -> Result<()> {
        let selected_table_name = self.selected_table_name().map(str::to_owned);
        let selected_table_index = self.selected_table;
        self.tables = self.db_ref()?.list_tables()?;
        self.sql.column_cache.clear();
        self.sql_invalidate_completion_cache();
        self.selected_table = selected_table_name
            .as_deref()
            .and_then(|table_name| {
                self.tables
                    .iter()
                    .position(|table| table.name == table_name)
            })
            .unwrap_or_else(|| selected_table_index.min(self.tables.len().saturating_sub(1)));
        self.details = None;
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
            self.details = None;
            self.detail = None;
            self.reset_content_position();
            self.refresh_preview()?;
        }
        Ok(())
    }

    pub(super) fn move_table_selection_down(&mut self) -> Result<()> {
        if self.selected_table + 1 < self.tables.len() {
            self.selected_table += 1;
            self.details = None;
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
        if self.is_home() {
            return Ok(());
        }

        if let Some(table_name) = self.selected_table_name().map(str::to_owned) {
            let db = self.db_ref()?;
            self.details = Some(db.table_details(&table_name)?);
            self.ensure_table_config();

            let queried_offset = self.row_offset;
            self.preview = self.db_ref()?.preview_table(
                &table_name,
                &self.visible_column_names(),
                &self.current_sort_clauses(),
                &self.current_filter_clauses(),
                self.row_limit,
                self.row_offset,
            )?;
            if let Some(details) = &mut self.details {
                details.total_rows = self.preview.total_rows;
            }
            self.clamp_row_viewport();
            if self.row_offset != queried_offset {
                self.preview = self.db_ref()?.preview_table(
                    &table_name,
                    &self.visible_column_names(),
                    &self.current_sort_clauses(),
                    &self.current_filter_clauses(),
                    self.row_limit,
                    self.row_offset,
                )?;
            }
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
        let total_rows = self.preview.total_rows;
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

    pub(super) fn db_ref(&self) -> Result<&Database> {
        self.db
            .as_ref()
            .ok_or_else(|| anyhow!("database is not loaded"))
    }

    pub(super) fn open_database(&mut self, path: &Path) -> Result<()> {
        let absolute_path = normalize_database_path(path)?;
        let db = Database::open(&absolute_path)?;
        let tables = db.list_tables()?;
        let mut warnings = Vec::new();
        let stored_session = match AppStorage::load_session(&absolute_path) {
            Ok(session) => session,
            Err(error) => {
                warnings.push(format!("could not restore session: {error}"));
                None
            }
        };

        self.mode = AppMode::Browse;
        self.path = Some(absolute_path.clone());
        self.db = Some(db);
        self.tables = tables;
        self.selected_table = 0;
        self.focus = PaneFocus::Tables;
        self.content_view = ContentView::Rows;
        self.preview = RowPreview::empty();
        self.details = None;
        self.detail = None;
        self.filter_modal = None;
        self.modal = None;
        self.search = None;
        self.status_message = None;
        self.sql.column_cache.clear();
        self.sql_invalidate_completion_cache();
        self.reset_content_position();
        self.restore_session_state(stored_session)?;
        match RecentStore::record(&absolute_path) {
            Ok(items) => {
                self.recent_items = items;
                if !self.recent_items.is_empty() {
                    self.selected_recent = self
                        .recent_items
                        .iter()
                        .position(|item| item.path == absolute_path)
                        .unwrap_or(0);
                }
            }
            Err(error) => warnings.push(format!("could not save recents: {error}")),
        }
        if !warnings.is_empty() {
            self.status_message = Some(format!("Opened database but {}", warnings.join("; ")));
        }
        Ok(())
    }

    fn handle_home(&mut self, action: Action) -> Result<()> {
        match action {
            Action::Quit => {}
            Action::ToggleFocus | Action::ReverseFocus | Action::MoveLeft | Action::MoveRight => {
                self.toggle_focus()
            }
            Action::MoveUp => self.move_up()?,
            Action::MoveDown => self.move_down()?,
            Action::Confirm => self.open_selected_recent(),
            Action::Delete => self.delete_selected_recent(),
            Action::Reload => {
                self.reload()?;
            }
            Action::None
            | Action::ToggleView
            | Action::OpenConfig
            | Action::CloseModal
            | Action::ToggleItem
            | Action::Clear
            | Action::EditDetail
            | Action::SaveDetail
            | Action::DiscardDetail
            | Action::OpenSearchCurrent
            | Action::OpenSearchAll
            | Action::OpenFilters
            | Action::SwitchToBrowse
            | Action::SwitchToSql
            | Action::MoveHome
            | Action::MoveEnd
            | Action::PageUp
            | Action::PageDown
            | Action::ExecuteSql
            | Action::NewLine
            | Action::InputChar(_)
            | Action::Backspace
            | Action::FollowLink => {}
        }

        Ok(())
    }

    pub(super) fn move_recent_selection_up(&mut self) {
        if self.selected_recent > 0 {
            self.selected_recent -= 1;
        }
    }

    pub(super) fn move_recent_selection_down(&mut self) {
        if self.selected_recent + 1 < self.recent_items.len() {
            self.selected_recent += 1;
        }
    }

    fn open_selected_recent(&mut self) {
        let Some(item) = self.selected_recent_item().cloned() else {
            return;
        };

        match self.open_database(&item.path) {
            Ok(()) => {}
            Err(error) => {
                self.status_message =
                    Some(format!("Could not open {}: {error}", item.path.display()));
                self.mode = AppMode::Home;
                self.db = None;
                self.path = None;
                self.tables.clear();
                self.details = None;
                self.preview = RowPreview::empty();
            }
        }
    }

    fn delete_selected_recent(&mut self) {
        let Some(item) = self.selected_recent_item().cloned() else {
            return;
        };

        match RecentStore::remove(&item.path) {
            Ok(items) => {
                self.recent_items = items;
                self.refresh_home_selection();
                self.status_message = Some(format!("Removed {} from recents", item.path.display()));
            }
            Err(error) => {
                self.status_message = Some(format!(
                    "Could not remove {} from recents: {error}",
                    item.path.display()
                ));
            }
        }
    }

    fn refresh_home_selection(&mut self) {
        if self.recent_items.is_empty() {
            self.selected_recent = 0;
            self.focus = PaneFocus::Content;
        } else {
            self.selected_recent = self
                .selected_recent
                .min(self.recent_items.len().saturating_sub(1));
            self.focus = PaneFocus::Tables;
        }
    }

    fn restore_session_state(&mut self, session: Option<StoredSession>) -> Result<()> {
        let Some(session) = session else {
            self.refresh_preview()?;
            return Ok(());
        };

        self.mode = match session.mode {
            AppMode::Home => AppMode::Browse,
            mode => mode,
        };
        self.focus = session.focus;
        self.content_view = session.content_view;
        self.sql.query = session.sql_query;
        self.sql.cursor = session.sql_cursor.min(self.sql.query.len());
        self.sql.focus = session.sql_focus;
        self.sql.history = session.sql_history;
        self.sql.selected_history = self.sql.history.len().saturating_sub(1);
        self.sql.result = SqlResultState::Empty;
        self.sql.result_scroll = 0;
        self.configs = self.restore_table_configs(&session.table_states)?;
        self.selected_table = session
            .selected_table_name
            .as_deref()
            .and_then(|table_name| {
                self.tables
                    .iter()
                    .position(|table| table.name == table_name)
            })
            .unwrap_or(0);
        self.selected_row = session.selected_row;
        self.row_offset = session.row_offset;
        self.schema_offset = session.schema_offset;
        self.refresh_preview()?;

        if let (Some(table_name), Some(rowid)) = (
            self.selected_table_name().map(str::to_owned),
            session.selected_row_rowid,
        ) && let Some(row_offset) = self.db_ref()?.locate_row_offset(
            &table_name,
            rowid,
            &self.current_sort_clauses(),
            &self.current_filter_clauses(),
        )? {
            self.selected_row = row_offset;
        }

        let queried_offset = self.row_offset;
        self.clamp_row_viewport();
        if self.row_offset != queried_offset {
            self.refresh_preview()?;
        }
        self.clamp_schema_offset();
        self.ensure_sql_viewport();
        Ok(())
    }

    fn restore_table_configs(
        &self,
        stored_tables: &[StoredTableState],
    ) -> Result<std::collections::HashMap<String, super::TableConfig>> {
        let mut configs = std::collections::HashMap::new();
        let Some(db) = &self.db else {
            return Ok(configs);
        };

        for table_state in stored_tables {
            if !self
                .tables
                .iter()
                .any(|table| table.name == table_state.table_name)
            {
                continue;
            }

            let columns = db.column_info(&table_state.table_name)?;
            let visible_columns = columns
                .iter()
                .map(|column| {
                    !table_state
                        .hidden_columns
                        .iter()
                        .any(|hidden| hidden == &column.name)
                })
                .collect::<Vec<_>>();
            let sort_clauses = table_state
                .sort_rules
                .iter()
                .filter_map(|rule| {
                    columns
                        .iter()
                        .position(|column| column.name == rule.column_name)
                        .map(|column_index| SortRule {
                            column_index,
                            descending: rule.descending,
                        })
                })
                .collect();
            let filter_rules = table_state
                .filter_rules
                .iter()
                .filter_map(|rule| {
                    columns
                        .iter()
                        .position(|column| column.name == rule.column_name)
                        .map(|column_index| FilterRule {
                            column_index,
                            mode: rule.mode,
                            value: rule.value.clone(),
                        })
                })
                .collect();

            configs.insert(
                table_state.table_name.clone(),
                super::TableConfig {
                    visible_columns,
                    sort_clauses,
                    filter_rules,
                },
            );
        }

        Ok(configs)
    }

    pub(super) fn persist_session_state(&self) -> Result<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        let Some(db) = &self.db else {
            return Ok(());
        };

        let selected_row_rowid = if let Some(table_name) = self.selected_table_name() {
            db.row_record_at_offset(
                table_name,
                &self.current_sort_clauses(),
                &self.current_filter_clauses(),
                self.selected_row,
            )?
            .and_then(|record| record.rowid)
        } else {
            None
        };

        let mut table_states = Vec::new();
        for (table_name, config) in &self.configs {
            let columns = match db.column_info(table_name) {
                Ok(columns) => columns,
                Err(_) => continue,
            };
            let hidden_columns = columns
                .iter()
                .zip(
                    config
                        .visible_columns
                        .iter()
                        .copied()
                        .chain(std::iter::repeat(true)),
                )
                .filter(|(_, visible)| !visible)
                .map(|(column, _)| column.name.clone())
                .collect::<Vec<_>>();
            let sort_rules = config
                .sort_clauses
                .iter()
                .filter_map(|rule| {
                    columns.get(rule.column_index).map(|column| StoredSortRule {
                        column_name: column.name.clone(),
                        descending: rule.descending,
                    })
                })
                .collect::<Vec<_>>();
            let filter_rules = config
                .filter_rules
                .iter()
                .filter_map(|rule| {
                    columns
                        .get(rule.column_index)
                        .map(|column| StoredFilterRule {
                            column_name: column.name.clone(),
                            mode: rule.mode,
                            value: rule.value.clone(),
                        })
                })
                .collect::<Vec<_>>();

            if hidden_columns.is_empty() && sort_rules.is_empty() && filter_rules.is_empty() {
                continue;
            }

            table_states.push(StoredTableState {
                table_name: table_name.clone(),
                hidden_columns,
                sort_rules,
                filter_rules,
            });
        }

        AppStorage::save_session(
            path,
            &StoredSession {
                mode: self.mode,
                focus: self.focus,
                content_view: self.content_view,
                selected_table_name: self.selected_table_name().map(str::to_owned),
                selected_row: self.selected_row,
                selected_row_rowid,
                row_offset: self.row_offset,
                schema_offset: self.schema_offset,
                sql_query: self.sql.query.clone(),
                sql_cursor: self.sql.cursor.min(self.sql.query.len()),
                sql_focus: self.sql.focus,
                sql_history: self.sql.history.clone(),
                table_states,
            },
        )
    }
}

fn split_table_name(table_name: &str) -> Option<(&str, &str)> {
    table_name.split_once('.')
}

#[cfg(test)]
mod tests;
