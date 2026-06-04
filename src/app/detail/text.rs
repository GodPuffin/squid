use super::super::{DetailField, DetailState};

pub(crate) fn detail_value_text(detail: &DetailState, field: &DetailField) -> String {
    let mut lines = Vec::new();

    if field.is_blob {
        lines.push("Blob preview (read-only).".to_string());
        lines.push(String::new());
    } else if detail.is_new_row {
        if detail.is_editing {
            lines.push("Editing new row field".to_string());
            lines.push(String::new());
        }
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

    if detail.is_new_row && (detail.is_editing || field.is_dirty()) {
        lines.push("Draft".to_string());
        if field.draft_value.is_empty() {
            lines.push("<empty>".to_string());
        } else {
            lines.extend(field.draft_value.lines().map(str::to_string));
        }
    } else if detail.is_editing || field.is_dirty() {
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
    } else if field.draft_value.is_empty() {
        lines.push("<empty>".to_string());
    } else {
        lines.extend(field.draft_value.lines().map(str::to_string));
    }

    lines.join("\n")
}

pub(crate) fn wrapped_line_count(value: &str, width: usize) -> usize {
    let width = width.max(1);
    let mut count = 0;

    for line in value.lines() {
        let chars = line.chars().count();
        count += chars.max(1).div_ceil(width);
    }

    if count == 0 { 1 } else { count }
}
