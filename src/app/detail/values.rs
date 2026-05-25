use std::collections::HashMap;

use rusqlite::types::Value;

use crate::db::ColumnInfo;

use super::super::DetailField;

pub(super) fn parse_detail_value(field: &DetailField) -> Result<Value, String> {
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
    if data_type.contains("REAL") || data_type.contains("FLOA") || data_type.contains("DOUB") {
        return input
            .parse::<f64>()
            .map(Value::Real)
            .map_err(|_| format!("{} expects a number", field.column_name));
    }
    if data_type.contains("NUM") || data_type.contains("DEC") {
        let numeric_input = input.trim();
        if numeric_input.parse::<i64>().is_ok() || numeric_input.parse::<f64>().is_ok() {
            return Ok(Value::Text(field.draft_value.clone()));
        }
        return Err(format!("{} expects a number", field.column_name));
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

pub(super) fn format_default_for_draft(default: &str) -> String {
    let trimmed = default.trim();
    if trimmed.len() >= 2
        && ((trimmed.starts_with('\'') && trimmed.ends_with('\''))
            || (trimmed.starts_with('"') && trimmed.ends_with('"')))
    {
        trimmed[1..trimmed.len() - 1].replace("''", "'")
    } else {
        trimmed.to_string()
    }
}

fn is_integer_primary_key(column: &ColumnInfo) -> bool {
    column.is_primary_key && column.data_type.to_ascii_uppercase().contains("INT")
}

fn should_omit_insert_column(field: &DetailField, column: &ColumnInfo) -> bool {
    if !field.draft_value.is_empty() {
        return false;
    }
    if is_integer_primary_key(column) {
        return true;
    }
    !field.not_null
}

pub(super) fn collect_insert_values(
    fields: &[DetailField],
    columns: &[ColumnInfo],
) -> Result<Vec<(String, Value)>, String> {
    let column_by_name: HashMap<&str, &ColumnInfo> = columns
        .iter()
        .map(|column| (column.name.as_str(), column))
        .collect();
    let mut insert_values = Vec::new();

    for field in fields {
        if field.is_blob {
            continue;
        }
        let Some(column) = column_by_name.get(field.column_name.as_str()) else {
            continue;
        };

        if should_omit_insert_column(field, column) {
            if field.not_null && column.default_value.is_none() {
                return Err(format!("{} is required", field.column_name));
            }
            continue;
        }

        if field.draft_value.is_empty() && field.not_null {
            return Err(format!("{} is required", field.column_name));
        }

        match parse_detail_value(field) {
            Ok(value) => insert_values.push((field.column_name.clone(), value)),
            Err(message) => return Err(message),
        }
    }

    for column in columns {
        if column.not_null
            && column.default_value.is_none()
            && !is_integer_primary_key(column)
            && !insert_values.iter().any(|(name, _)| name == &column.name)
        {
            return Err(format!("{} is required", column.name));
        }
    }

    Ok(insert_values)
}
