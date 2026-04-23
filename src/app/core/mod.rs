mod load;
mod open;
mod reload;
mod selection;
mod session;
mod viewport;

use std::path::PathBuf;

use anyhow::{Result, anyhow};

use crate::db::Database;

use super::{Action, App, AppMode, ContentView, PaneFocus};

impl App {
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
            self.close_search();
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

    pub(in crate::app) fn db_ref(&self) -> Result<&Database> {
        self.db
            .as_ref()
            .ok_or_else(|| anyhow!("database is not loaded"))
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

    fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            PaneFocus::Tables => PaneFocus::Content,
            PaneFocus::Content => PaneFocus::Tables,
        };
    }

    fn toggle_view(&mut self) {
        if self.is_home() {
            return;
        }
        self.detail = None;
        self.content_view = match self.content_view {
            ContentView::Rows => ContentView::Schema,
            ContentView::Schema => ContentView::Rows,
        };
    }

    fn move_up(&mut self) -> Result<()> {
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

    fn move_down(&mut self) -> Result<()> {
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
}

fn split_table_name(table_name: &str) -> Option<(&str, &str)> {
    table_name.split_once('.')
}

#[cfg(test)]
mod tests;
