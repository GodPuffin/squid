use std::path::Path;

use anyhow::Result;

use crate::db::{Database, RowPreview};

use super::super::{
    App, AppMode, ContentView, PaneFocus, RecentStore,
    home::{AppStorage, normalize_database_path},
};

impl App {
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

    pub(in crate::app) fn move_recent_selection_up(&mut self) {
        if self.selected_recent > 0 {
            self.selected_recent -= 1;
        }
    }

    pub(in crate::app) fn move_recent_selection_down(&mut self) {
        if self.selected_recent + 1 < self.recent_items.len() {
            self.selected_recent += 1;
        }
    }

    pub(super) fn open_selected_recent(&mut self) {
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

    pub(super) fn delete_selected_recent(&mut self) {
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

    pub(super) fn refresh_home_selection(&mut self) {
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
}
