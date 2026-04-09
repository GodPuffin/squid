use anyhow::Result;
use rusqlite::types::Value;

use super::{
    Action, App, ContentView, DetailField, DetailForeignTarget, DetailMessage, DetailPane,
    DetailState,
};

impl App {
    pub fn detail_select_field(&mut self, index: usize) {
        let Some(detail) = &mut self.detail else {
            return;
        };
        if detail.fields.is_empty() {
            return;
        }

        detail.pane = DetailPane::Fields;
        detail.selected_field = index.min(detail.fields.len().saturating_sub(1));
        detail.value_scroll = 0;
        detail.is_editing = false;
        detail.message = None;
    }

    pub fn detail_focus_value(&mut self) {
        if let Some(detail) = &mut self.detail {
            detail.pane = DetailPane::Value;
        }
    }

    pub fn detail_scroll_value(&mut self, delta: isize) {
        if delta < 0 {
            self.detail_move_up();
        } else if delta > 0 {
            self.detail_move_down();
        }
    }

    pub fn detail_is_editing(&self) -> bool {
        self.detail.as_ref().is_some_and(|detail| detail.is_editing)
    }

    pub fn detail_has_changes(&self) -> bool {
        self.detail
            .as_ref()
            .is_some_and(|detail| detail.fields.iter().any(DetailField::is_dirty))
    }

    pub fn detail_is_row_writable(&self) -> bool {
        self.detail
            .as_ref()
            .and_then(|detail| detail.rowid)
            .is_some()
    }

    pub fn detail_selected_field_is_editable(&self) -> bool {
        self.detail
            .as_ref()
            .and_then(|detail| detail.fields.get(detail.selected_field))
            .is_some_and(|field| !field.is_blob)
            && self.detail_is_row_writable()
    }

    pub fn detail_pane(&self) -> Option<DetailPane> {
        self.detail.as_ref().map(|detail| detail.pane)
    }

    pub(super) fn handle_detail(&mut self, action: Action) -> Result<()> {
        match action {
            Action::CloseModal | Action::Quit => self.detail = None,
            Action::ReverseFocus => self.detail_move_left(),
            Action::MoveLeft => self.detail_move_left(),
            Action::MoveRight | Action::ToggleFocus => self.detail_move_right(),
            Action::MoveUp => self.detail_move_up(),
            Action::MoveDown => self.detail_move_down(),
            Action::FollowLink | Action::Confirm => self.follow_detail_link()?,
            Action::EditDetail | Action::ToggleItem => self.toggle_detail_editing(),
            Action::SaveDetail => self.save_detail_changes()?,
            Action::DiscardDetail => self.discard_detail_changes(),
            Action::InputChar(ch) => self.detail_input_char(ch),
            Action::Backspace => self.detail_backspace(),
            Action::NewLine => self.detail_insert_newline(),
            Action::None
            | Action::SwitchToBrowse
            | Action::SwitchToSql
            | Action::ToggleView
            | Action::MoveHome
            | Action::MoveEnd
            | Action::PageUp
            | Action::PageDown
            | Action::OpenConfig
            | Action::Delete
            | Action::Clear
            | Action::Reload
            | Action::OpenSearchCurrent
            | Action::OpenSearchAll
            | Action::OpenFilters
            | Action::ExecuteSql => {}
        }

        Ok(())
    }

    pub(super) fn open_detail(&mut self) -> Result<()> {
        if self.focus != super::PaneFocus::Content || self.content_view != ContentView::Rows {
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
            rowid,
            row_label: record.row_label,
            pane: DetailPane::Fields,
            selected_field: 0,
            value_scroll: 0,
            value_view_width: super::DEFAULT_DETAIL_VALUE_WIDTH,
            value_view_height: super::DEFAULT_DETAIL_VALUE_HEIGHT,
            is_editing: false,
            message: rowid.is_none().then(|| DetailMessage {
                text: "Read-only row: rowid is unavailable for this table view".to_string(),
                is_error: false,
            }),
            fields,
        });

        Ok(())
    }

    pub(super) fn clamp_detail_scroll(&mut self) {
        let Some(detail) = &mut self.detail else {
            return;
        };
        if detail.fields.is_empty() {
            detail.selected_field = 0;
            detail.value_scroll = 0;
            return;
        }

        detail.selected_field = detail
            .selected_field
            .min(detail.fields.len().saturating_sub(1));
        let value = detail_value_text(detail, &detail.fields[detail.selected_field]);
        let line_count = wrapped_line_count(&value, detail.value_view_width);
        let max_scroll = line_count.saturating_sub(detail.value_view_height);
        detail.value_scroll = detail.value_scroll.min(max_scroll);
    }

    fn detail_move_left(&mut self) {
        if let Some(detail) = &mut self.detail {
            detail.pane = DetailPane::Fields;
            detail.is_editing = false;
        }
    }

    fn detail_move_right(&mut self) {
        if let Some(detail) = &mut self.detail {
            detail.pane = DetailPane::Value;
        }
    }

    fn detail_move_up(&mut self) {
        let Some(detail) = &mut self.detail else {
            return;
        };
        if detail.is_editing {
            return;
        }
        match detail.pane {
            DetailPane::Fields => {
                detail.selected_field = detail.selected_field.saturating_sub(1);
                detail.value_scroll = 0;
            }
            DetailPane::Value => {
                detail.value_scroll = detail.value_scroll.saturating_sub(1);
            }
        }
    }

    fn detail_move_down(&mut self) {
        let Some(detail) = &mut self.detail else {
            return;
        };
        if detail.is_editing {
            return;
        }
        match detail.pane {
            DetailPane::Fields => {
                if !detail.fields.is_empty() {
                    detail.selected_field =
                        (detail.selected_field + 1).min(detail.fields.len().saturating_sub(1));
                    detail.value_scroll = 0;
                }
            }
            DetailPane::Value => {
                detail.value_scroll = detail.value_scroll.saturating_add(1);
            }
        }
        self.clamp_detail_scroll();
    }

    fn toggle_detail_editing(&mut self) {
        let Some(detail) = &mut self.detail else {
            return;
        };

        detail.message = None;
        if detail.is_editing {
            detail.is_editing = false;
            detail.pane = DetailPane::Value;
            self.clamp_detail_scroll();
            return;
        }

        let Some(field) = detail.fields.get(detail.selected_field) else {
            return;
        };

        if detail.rowid.is_none() {
            detail.message = Some(DetailMessage {
                text: "This row is read-only and cannot be edited".to_string(),
                is_error: true,
            });
            return;
        }
        if field.is_blob {
            detail.message = Some(DetailMessage {
                text: "Blob values are read-only in the details modal".to_string(),
                is_error: true,
            });
            return;
        }

        detail.pane = DetailPane::Value;
        detail.is_editing = true;
        detail.value_scroll = 0;
        self.clamp_detail_scroll();
    }

    fn detail_input_char(&mut self, ch: char) {
        let Some(detail) = &mut self.detail else {
            return;
        };
        if !detail.is_editing {
            return;
        }

        if let Some(field) = detail.fields.get_mut(detail.selected_field) {
            field.draft_value.push(ch);
            detail.message = None;
        }
        self.clamp_detail_scroll();
    }

    fn detail_backspace(&mut self) {
        let Some(detail) = &mut self.detail else {
            return;
        };
        if !detail.is_editing {
            return;
        }

        if let Some(field) = detail.fields.get_mut(detail.selected_field) {
            field.draft_value.pop();
            detail.message = None;
        }
        self.clamp_detail_scroll();
    }

    fn detail_insert_newline(&mut self) {
        let Some(detail) = &mut self.detail else {
            return;
        };
        if !detail.is_editing {
            return;
        }

        if let Some(field) = detail.fields.get_mut(detail.selected_field) {
            field.draft_value.push('\n');
            detail.message = None;
        }
        self.clamp_detail_scroll();
    }

    fn discard_detail_changes(&mut self) {
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

    fn save_detail_changes(&mut self) -> Result<()> {
        let Some(table_name) = self.selected_table_name().map(str::to_owned) else {
            return Ok(());
        };
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

    fn follow_detail_link(&mut self) -> Result<()> {
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

pub(crate) fn detail_value_text(detail: &DetailState, field: &DetailField) -> String {
    let mut lines = Vec::new();

    if field.is_blob {
        lines.push("Blob values are displayed read-only.".to_string());
        lines.push(String::new());
    } else if detail.rowid.is_none() {
        lines.push("This row is read-only because rowid is unavailable.".to_string());
        lines.push(String::new());
    } else if detail.is_editing {
        lines.push("Editing current field".to_string());
        lines.push(String::new());
    }

    lines.push(format!(
        "Type: {}{}",
        if field.data_type.is_empty() {
            "TEXT"
        } else {
            field.data_type.as_str()
        },
        if field.not_null { "  NOT NULL" } else { "" }
    ));
    lines.push(String::new());

    if detail.is_editing || field.is_dirty() {
        lines.push("Original".to_string());
        if field.original_value.is_empty() {
            lines.push("<empty>".to_string());
        } else {
            lines.extend(field.original_value.lines().map(str::to_string));
        }

        lines.push(String::new());
        lines.push("Draft".to_string());
        if field.draft_value.is_empty() {
            lines.push("<empty>".to_string());
        } else {
            lines.extend(field.draft_value.lines().map(str::to_string));
        }
    } else {
        if field.draft_value.is_empty() {
            lines.push("<empty>".to_string());
        } else {
            lines.extend(field.draft_value.lines().map(str::to_string));
        }
    }

    lines.join("\n")
}

fn wrapped_line_count(value: &str, width: usize) -> usize {
    let width = width.max(1);
    let mut count = 0;

    for line in value.lines() {
        let chars = line.chars().count();
        count += chars.max(1).div_ceil(width);
    }

    if count == 0 { 1 } else { count }
}

fn parse_detail_value(field: &DetailField) -> Result<Value, String> {
    let input = field.draft_value.as_str();
    if input == "NULL" {
        if field.not_null {
            return Err(format!("{} cannot be NULL", field.column_name));
        }
        return Ok(Value::Null);
    }

    let data_type = field.data_type.to_ascii_uppercase();
    if data_type.contains("BOOL") {
        return parse_bool_value(field.column_name.as_str(), input);
    }
    if data_type.contains("INT") {
        return input
            .parse::<i64>()
            .map(Value::Integer)
            .map_err(|_| format!("{} expects an integer", field.column_name));
    }
    if data_type.contains("REAL")
        || data_type.contains("FLOA")
        || data_type.contains("DOUB")
        || data_type.contains("NUM")
        || data_type.contains("DEC")
    {
        return input
            .parse::<f64>()
            .map(Value::Real)
            .map_err(|_| format!("{} expects a number", field.column_name));
    }

    Ok(Value::Text(field.draft_value.clone()))
}

fn parse_bool_value(column_name: &str, input: &str) -> Result<Value, String> {
    match input.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "t" | "yes" | "y" | "on" => Ok(Value::Integer(1)),
        "0" | "false" | "f" | "no" | "n" | "off" => Ok(Value::Integer(0)),
        _ => Err(format!("{column_name} expects true/false or 1/0")),
    }
}

#[cfg(test)]
mod tests;
