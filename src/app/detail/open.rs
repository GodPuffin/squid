use anyhow::Result;

use super::super::{
    App, ContentView, DetailField, DetailForeignTarget, DetailMessage, DetailPane, DetailState,
};
use super::values::format_default_for_draft;

impl App {
    pub(in crate::app) fn open_detail(&mut self) -> Result<()> {
        if self.focus != super::super::PaneFocus::Content || self.content_view != ContentView::Rows
        {
            return Ok(());
        }
        let Some(table_name) = self.selected_table_name().map(str::to_owned) else {
            return Ok(());
        };
        if self.preview.total_rows == 0 {
            return Ok(());
        }

        let record = self.db_ref()?.row_record_at_offset(
            &table_name,
            &self.current_sort_clauses(),
            &self.current_filter_clauses(),
            self.selected_row,
        )?;
        let Some(record) = record else {
            return Ok(());
        };
        let table_is_writable = self.db_ref()?.table_is_writable(&table_name)?;

        let rowid = record.rowid;
        let column_meta = self
            .details
            .as_ref()
            .map(|details| {
                details
                    .columns
                    .iter()
                    .map(|column| {
                        (
                            column.name.clone(),
                            (column.data_type.clone(), column.not_null),
                        )
                    })
                    .collect::<std::collections::HashMap<_, _>>()
            })
            .unwrap_or_default();
        let fields = record
            .fields
            .into_iter()
            .map(|field| {
                let (data_type, not_null) = column_meta
                    .get(&field.column_name)
                    .cloned()
                    .unwrap_or_else(|| (String::new(), false));
                let foreign_target = record
                    .foreign_keys
                    .iter()
                    .find(|fk| fk.from_column == field.column_name)
                    .and_then(|fk| {
                        if field.value == "NULL" {
                            None
                        } else {
                            Some(DetailForeignTarget {
                                table_name: fk.target_table.clone(),
                                column_name: fk.target_column.clone(),
                                value: field.value.clone(),
                            })
                        }
                    });
                DetailField {
                    column_name: field.column_name,
                    data_type,
                    not_null,
                    original_value: field.value.clone(),
                    draft_value: field.value,
                    foreign_target,
                    is_blob: field.is_blob,
                }
            })
            .collect();

        self.detail = Some(DetailState {
            is_new_row: false,
            rowid,
            row_label: record.row_label,
            pane: DetailPane::Fields,
            selected_field: 0,
            value_scroll: 0,
            value_view_width: super::super::DEFAULT_DETAIL_VALUE_WIDTH,
            value_view_height: super::super::DEFAULT_DETAIL_VALUE_HEIGHT,
            is_editing: false,
            message: match (rowid, table_is_writable) {
                (None, _) => Some(DetailMessage {
                    text: "Read-only row: rowid is unavailable for this table view".to_string(),
                    is_error: false,
                }),
                (Some(_), false) => Some(DetailMessage {
                    text: "Read-only database: this row cannot be edited".to_string(),
                    is_error: false,
                }),
                (Some(_), true) => None,
            },
            fields,
        });

        Ok(())
    }

    pub(in crate::app) fn open_new_row(&mut self) -> Result<()> {
        if self.focus != super::super::PaneFocus::Content || self.content_view != ContentView::Rows
        {
            return Ok(());
        }
        let Some(table_name) = self.selected_table_name().map(str::to_owned) else {
            return Ok(());
        };
        if !self.db_ref()?.table_is_writable(&table_name)? {
            self.status_message =
                Some("Cannot add rows because this database is read-only".to_string());
            return Ok(());
        }

        let column_info = self.db_ref()?.column_info(&table_name)?;
        if column_info.is_empty() {
            return Ok(());
        }

        let fields = column_info
            .iter()
            .map(|column| {
                let initial_value = column
                    .default_value
                    .as_deref()
                    .map(format_default_for_draft)
                    .unwrap_or_default();
                DetailField {
                    column_name: column.name.clone(),
                    data_type: column.data_type.clone(),
                    not_null: column.not_null,
                    original_value: initial_value.clone(),
                    draft_value: initial_value,
                    foreign_target: None,
                    is_blob: column.data_type.to_ascii_uppercase().contains("BLOB"),
                }
            })
            .collect();

        self.detail = Some(DetailState {
            is_new_row: true,
            rowid: None,
            row_label: "New row".to_string(),
            pane: DetailPane::Fields,
            selected_field: 0,
            value_scroll: 0,
            value_view_width: super::super::DEFAULT_DETAIL_VALUE_WIDTH,
            value_view_height: super::super::DEFAULT_DETAIL_VALUE_HEIGHT,
            is_editing: false,
            message: Some(DetailMessage {
                text: "Fill fields, then press s to insert".to_string(),
                is_error: false,
            }),
            fields,
        });

        Ok(())
    }
}
