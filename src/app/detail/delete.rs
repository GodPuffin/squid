use anyhow::Result;

use super::super::{App, ContentView, DetailMessage, PaneFocus, PendingRowDelete};

impl App {
    pub(in crate::app) fn clear_pending_row_delete(&mut self) {
        self.pending_row_delete = None;
    }

    pub(in crate::app) fn delete_selected_row(&mut self) -> Result<()> {
        if self.detail.as_ref().is_some_and(|detail| detail.is_new_row) {
            return Ok(());
        }
        if self.detail_has_changes() {
            let message = "Discard or save edits before deleting this row".to_string();
            if let Some(detail) = &mut self.detail {
                detail.message = Some(DetailMessage {
                    text: message,
                    is_error: true,
                });
            } else {
                self.status_message = Some(message);
            }
            return Ok(());
        }

        let Some(table_name) = self.selected_table_name().map(str::to_owned) else {
            return Ok(());
        };
        if !self.db_ref()?.table_is_writable(&table_name)? {
            self.status_message =
                Some("Cannot delete rows because this database is read-only".to_string());
            return Ok(());
        }

        let rowid = if let Some(rowid) = self.detail.as_ref().and_then(|detail| detail.rowid) {
            rowid
        } else {
            if self.focus != PaneFocus::Content
                || self.content_view != ContentView::Rows
                || self.preview.total_rows == 0
            {
                return Ok(());
            }
            let Some(record) = self.db_ref()?.row_record_at_offset(
                &table_name,
                &self.current_sort_clauses(),
                &self.current_filter_clauses(),
                self.selected_row,
            )?
            else {
                return Ok(());
            };
            let Some(rowid) = record.rowid else {
                self.status_message =
                    Some("Cannot delete this row because rowid is unavailable".to_string());
                return Ok(());
            };
            rowid
        };

        let target = PendingRowDelete {
            table_name: table_name.clone(),
            rowid,
        };

        if self.app_settings.confirm_before_delete_row {
            let pending = self.pending_row_delete.as_ref();
            if pending.is_some_and(|pending| pending == &target) {
                self.clear_pending_row_delete();
            } else {
                self.pending_row_delete = Some(target);
                self.status_message = Some(format!(
                    "Press d again to delete row {rowid} from {table_name}",
                ));
                return Ok(());
            }
        } else {
            self.clear_pending_row_delete();
        }

        match self.db_ref()?.delete_row(&table_name, rowid) {
            Ok(()) => {
                self.detail = None;
                self.refresh_preview()?;
                self.status_message =
                    Some(format!("Deleted row {rowid} from {table_name}"));
            }
            Err(err) => {
                let message = format!("Could not delete row: {err}");
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

        Ok(())
    }
}
