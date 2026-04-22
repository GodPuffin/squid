use anyhow::Result;

use super::super::{App, RecentStore};

impl App {
    pub(in crate::app) fn reload(&mut self) -> Result<()> {
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

    pub(in crate::app) fn refresh_loaded_db_state(&mut self) -> Result<()> {
        let selected_table_name = self.selected_table_name().map(str::to_owned);
        let selected_table_index = self.selected_table;
        self.db_ref()?.clear_caches();
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
}
