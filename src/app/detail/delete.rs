use anyhow::Result;

use super::super::{App, ContentView, DetailMessage, PaneFocus};

impl App {
    pub(in crate::app) fn delete_selected_row(&mut self) -> Result<()> {
        if let Some(message) = self.delete_row_blocked_reason() {
            self.set_delete_feedback(message);
            return Ok(());
        }

        if self.detail.as_ref().is_some_and(|detail| detail.is_new_row) {
            return Ok(());
        }
        if self.detail_has_changes() {
            self.set_delete_feedback("Discard or save edits before deleting this row".to_string());
            return Ok(());
        }

        let Some(table_name) = self.selected_table_name().map(str::to_owned) else {
            return Ok(());
        };
        if !self.db_ref()?.table_is_writable(&table_name)? {
            self.set_delete_feedback(
                "Cannot delete rows because this database is read-only".to_string(),
            );
            return Ok(());
        }

        let rowid = if let Some(rowid) = self.detail.as_ref().and_then(|detail| detail.rowid) {
            rowid
        } else {
            let Some(record) = self.db_ref()?.row_record_at_offset(
                &table_name,
                &self.current_sort_clauses(),
                &self.current_filter_clauses(),
                self.selected_row,
            )?
            else {
                self.set_delete_feedback("No row to delete".to_string());
                return Ok(());
            };
            let Some(rowid) = record.rowid else {
                self.set_delete_feedback(
                    "Cannot delete this row because rowid is unavailable".to_string(),
                );
                return Ok(());
            };
            rowid
        };

        match self.db_ref()?.delete_row(&table_name, rowid) {
            Ok(()) => {
                self.detail = None;
                self.refresh_preview()?;
                self.status_message = Some(format!("Deleted row {rowid} from {table_name}"));
            }
            Err(err) => {
                self.set_delete_feedback(format!("Could not delete row: {err}"));
            }
        }

        Ok(())
    }

    fn delete_row_blocked_reason(&self) -> Option<String> {
        if self.detail.is_some() {
            return None;
        }

        if self.selected_table_name().is_none() {
            return Some("Select a table before deleting a row".to_string());
        }

        if self.content_view == ContentView::Schema {
            return Some("Switch to row view (v) to delete rows".to_string());
        }

        if self.focus != PaneFocus::Content {
            return Some("Press Tab to focus the row preview before deleting".to_string());
        }

        if self.preview.total_rows == 0 {
            return Some("No row to delete".to_string());
        }

        if !self.table_has_rowid_alias() {
            return Some("Cannot delete rows because this table has no rowid".to_string());
        }

        if !self.detail_database_is_writable() {
            return Some("Cannot delete rows because this database is read-only".to_string());
        }

        None
    }

    fn set_delete_feedback(&mut self, message: String) {
        if let Some(detail) = &mut self.detail {
            detail.message = Some(DetailMessage {
                text: message,
                is_error: true,
            });
        } else {
            self.status_message = Some(message);
        }
    }
}
