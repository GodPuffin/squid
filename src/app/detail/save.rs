use anyhow::Result;

use super::super::{App, DetailMessage, DetailPane};
use super::values::{collect_insert_values, parse_detail_value};

impl App {
    pub(super) fn discard_detail_changes(&mut self) {
        let Some(detail) = &mut self.detail else {
            return;
        };

        let dirty_fields = detail
            .fields
            .iter()
            .filter(|field| field.is_dirty())
            .count();
        for field in &mut detail.fields {
            field.draft_value = field.original_value.clone();
        }
        detail.is_editing = false;
        detail.message = Some(DetailMessage {
            text: if dirty_fields == 0 {
                "No pending row edits".to_string()
            } else {
                format!("Discarded {dirty_fields} field edit(s)")
            },
            is_error: false,
        });
        self.clamp_detail_scroll();
    }

    pub(super) fn save_detail_changes(&mut self) -> Result<()> {
        let Some(table_name) = self.selected_table_name().map(str::to_owned) else {
            return Ok(());
        };
        let is_new_row = self.detail.as_ref().is_some_and(|detail| detail.is_new_row);
        if !self.detail_database_is_writable() {
            if let Some(detail) = &mut self.detail {
                detail.message = Some(DetailMessage {
                    text: if is_new_row {
                        "This row cannot be inserted because the database is read-only".to_string()
                    } else {
                        "This row cannot be saved because the database is read-only".to_string()
                    },
                    is_error: true,
                });
            }
            return Ok(());
        }

        if is_new_row {
            return self.insert_new_row(&table_name);
        }

        let Some(detail) = &self.detail else {
            return Ok(());
        };
        let Some(rowid) = detail.rowid else {
            if let Some(detail) = &mut self.detail {
                detail.message = Some(DetailMessage {
                    text: "This row cannot be saved because rowid is unavailable".to_string(),
                    is_error: true,
                });
            }
            return Ok(());
        };

        let mut changes = Vec::new();
        for field in detail
            .fields
            .iter()
            .filter(|field| field.is_dirty() && !field.is_blob)
        {
            match parse_detail_value(field) {
                Ok(value) => changes.push((field.column_name.clone(), value)),
                Err(message) => {
                    if let Some(detail) = &mut self.detail {
                        detail.message = Some(DetailMessage {
                            text: message,
                            is_error: true,
                        });
                    }
                    return Ok(());
                }
            }
        }
        if changes.is_empty() {
            if let Some(detail) = &mut self.detail {
                detail.is_editing = false;
                detail.message = Some(DetailMessage {
                    text: "No pending row edits".to_string(),
                    is_error: false,
                });
            }
            return Ok(());
        }

        let selected_field = detail.selected_field;

        match self
            .db_ref()?
            .update_row_values(&table_name, rowid, &changes)
        {
            Ok(updated_rowid) => {
                let offset = self.db_ref()?.locate_row_offset(
                    &table_name,
                    updated_rowid,
                    &self.current_sort_clauses(),
                    &self.current_filter_clauses(),
                )?;
                if let Some(offset) = offset {
                    self.jump_to_row_offset(offset)?;
                    self.detail = None;
                    self.open_detail()?;
                    if let Some(detail) = &mut self.detail {
                        detail.selected_field =
                            selected_field.min(detail.fields.len().saturating_sub(1));
                        detail.pane = DetailPane::Value;
                        detail.is_editing = false;
                        detail.message = Some(DetailMessage {
                            text: format!("Saved {} field(s)", changes.len()),
                            is_error: false,
                        });
                    }
                    self.clamp_detail_scroll();
                } else {
                    self.detail = None;
                    self.refresh_preview()?;
                    self.status_message = Some(format!(
                        "Saved {} field(s); row no longer matches current view",
                        changes.len()
                    ));
                }
            }
            Err(err) => {
                if let Some(detail) = &mut self.detail {
                    detail.message = Some(DetailMessage {
                        text: format!("Could not save row: {err}"),
                        is_error: true,
                    });
                }
            }
        }

        Ok(())
    }

    fn insert_new_row(&mut self, table_name: &str) -> Result<()> {
        let column_info = self.db_ref()?.column_info(table_name)?;
        let fields = self
            .detail
            .as_ref()
            .map(|detail| detail.fields.as_slice())
            .unwrap_or_default();
        let selected_field = self
            .detail
            .as_ref()
            .map(|detail| detail.selected_field)
            .unwrap_or(0);

        let values = match collect_insert_values(fields, &column_info) {
            Ok(values) => values,
            Err(message) => {
                if let Some(detail) = &mut self.detail {
                    detail.message = Some(DetailMessage {
                        text: message,
                        is_error: true,
                    });
                }
                return Ok(());
            }
        };
        if values.is_empty() {
            if let Some(detail) = &mut self.detail {
                detail.is_editing = false;
                detail.message = Some(DetailMessage {
                    text: "No values to insert".to_string(),
                    is_error: false,
                });
            }
            return Ok(());
        }

        match self.db_ref()?.insert_row_values(table_name, &values) {
            Ok(Some(inserted_rowid)) => {
                let offset = self.db_ref()?.locate_row_offset(
                    table_name,
                    inserted_rowid,
                    &self.current_sort_clauses(),
                    &self.current_filter_clauses(),
                )?;
                if let Some(offset) = offset {
                    self.jump_to_row_offset(offset)?;
                    self.detail = None;
                    self.open_detail()?;
                    if let Some(detail) = &mut self.detail {
                        detail.selected_field =
                            selected_field.min(detail.fields.len().saturating_sub(1));
                        detail.pane = DetailPane::Value;
                        detail.is_editing = false;
                        detail.message = Some(DetailMessage {
                            text: format!("Inserted {} field(s)", values.len()),
                            is_error: false,
                        });
                    }
                    self.clamp_detail_scroll();
                } else {
                    self.detail = None;
                    self.refresh_preview()?;
                    self.status_message = Some(format!(
                        "Inserted {} field(s); row no longer matches current view",
                        values.len()
                    ));
                }
            }
            Ok(None) => {
                self.detail = None;
                self.refresh_preview()?;
                self.status_message = Some(format!("Inserted {} field(s)", values.len()));
            }
            Err(err) => {
                if let Some(detail) = &mut self.detail {
                    detail.message = Some(DetailMessage {
                        text: format!("Could not insert row: {err}"),
                        is_error: true,
                    });
                }
            }
        }

        Ok(())
    }

    pub(crate) fn follow_detail_link(&mut self) -> Result<()> {
        let target = self
            .detail
            .as_ref()
            .filter(|detail| !detail.is_editing)
            .and_then(|detail| detail.fields.get(detail.selected_field))
            .and_then(|field| field.foreign_target.clone());
        let Some(target) = target else {
            return Ok(());
        };

        if !self.select_table_by_name(&target.table_name)? {
            return Ok(());
        }

        self.detail = None;
        let Some(offset) = self.db_ref()?.locate_foreign_row_offset(
            &target.table_name,
            &target.column_name,
            &target.value,
            &self.current_sort_clauses(),
            &self.current_filter_clauses(),
        )?
        else {
            return Ok(());
        };

        self.jump_to_row_offset(offset)
    }
}
