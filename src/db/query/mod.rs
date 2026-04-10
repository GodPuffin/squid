use anyhow::{Result, anyhow};
use rusqlite::params_from_iter;
use rusqlite::types::Value;

use super::schema::count_rows;
use super::value::format_value;
use super::{
    ColumnInfo, Database, FilterClause, FilterMode, RowField, RowPreview, RowRecord, SortClause,
};

impl Database {
    pub fn preview_table(
        &self,
        table_name: &str,
        visible_columns: &[String],
        sort_clauses: &[SortClause],
        filter_clauses: &[FilterClause],
        limit: usize,
        offset: usize,
    ) -> Result<RowPreview> {
        let rowid_alias = self.rowid_alias(table_name)?;
        let safe_table_name = quote_table_name(table_name);
        let columns = if visible_columns.is_empty() {
            self.list_columns(table_name)?
        } else {
            visible_columns.to_vec()
        };
        let (where_clause, mut filter_params) = build_filter_where(filter_clauses);
        let total_rows = count_rows(&self.conn, &safe_table_name, &where_clause, &filter_params)?;
        let select_list = columns
            .iter()
            .map(|column| quote_identifier(column))
            .collect::<Vec<_>>()
            .join(", ");
        let order_by = build_order_by(sort_clauses);
        let Some(rowid_alias) = rowid_alias else {
            let sql = format!(
                "SELECT {select_list} FROM {safe_table_name}{where_clause}{order_by} LIMIT ? OFFSET ?"
            );
            filter_params.push(Value::Integer(limit as i64));
            filter_params.push(Value::Integer(offset as i64));
            let mut stmt = self.conn.prepare(&sql)?;
            let column_count = stmt.column_count();
            let row_iter = stmt
                .query_map(params_from_iter(filter_params.iter()), |row| {
                    let mut values = Vec::with_capacity(column_count);
                    for idx in 0..column_count {
                        values.push(format_value(row.get_ref(idx)?));
                    }
                    Ok((None::<i64>, values))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            return Ok(RowPreview {
                columns,
                rows: row_iter.into_iter().map(|(_, values)| values).collect(),
                total_rows,
            });
        };
        let sql = format!(
            "SELECT {rowid_alias}, {select_list} FROM {safe_table_name}{where_clause}{order_by} LIMIT ? OFFSET ?"
        );
        filter_params.push(Value::Integer(limit as i64));
        filter_params.push(Value::Integer(offset as i64));
        let row_iter = match self.conn.prepare(&sql) {
            Ok(mut stmt) => {
                let column_count = stmt.column_count().saturating_sub(1);
                stmt.query_map(params_from_iter(filter_params.iter()), |row| {
                    let rowid = row.get::<_, i64>(0)?;
                    let mut values = Vec::with_capacity(column_count);
                    for idx in 0..column_count {
                        values.push(format_value(row.get_ref(idx + 1)?));
                    }
                    Ok((Some(rowid), values))
                })?
                .collect::<Result<Vec<_>, _>>()?
            }
            Err(_) => {
                let fallback_sql = format!(
                    "SELECT {select_list} FROM {safe_table_name}{where_clause}{order_by} LIMIT ? OFFSET ?"
                );
                let mut stmt = self.conn.prepare(&fallback_sql)?;
                let column_count = stmt.column_count();
                stmt.query_map(params_from_iter(filter_params.iter()), |row| {
                    let mut values = Vec::with_capacity(column_count);
                    for idx in 0..column_count {
                        values.push(format_value(row.get_ref(idx)?));
                    }
                    Ok((None::<i64>, values))
                })?
                .collect::<Result<Vec<_>, _>>()?
            }
        };

        Ok(RowPreview {
            columns,
            rows: row_iter
                .into_iter()
                .map(|(_row_id, values)| values)
                .collect(),
            total_rows,
        })
    }

    pub fn row_record_at_offset(
        &self,
        table_name: &str,
        sort_clauses: &[SortClause],
        filter_clauses: &[FilterClause],
        offset: usize,
    ) -> Result<Option<RowRecord>> {
        let column_info = self.column_info(table_name)?;
        if column_info.is_empty() {
            return Ok(None);
        }
        let columns = column_info
            .iter()
            .map(|column| column.name.clone())
            .collect::<Vec<_>>();

        let safe_table_name = quote_table_name(table_name);
        let select_list = columns
            .iter()
            .map(|column| quote_identifier(column))
            .collect::<Vec<_>>()
            .join(", ");
        let Some(rowid_alias) = rowid_alias_from_columns(&column_info) else {
            let (where_clause, mut filter_params) = build_filter_where(filter_clauses);
            let fallback_sql = format!(
                "SELECT {select_list}
                 FROM {safe_table_name}
                 {where_clause}
                 {}
                 LIMIT 1 OFFSET ?",
                build_order_by(sort_clauses)
            );
            filter_params.push(Value::Integer(offset as i64));
            let mut fallback_stmt = self.conn.prepare(&fallback_sql)?;
            let mut rows = fallback_stmt.query(params_from_iter(filter_params.iter()))?;
            let Some(row) = rows.next()? else {
                return Ok(None);
            };
            let fields = columns
                .iter()
                .enumerate()
                .map(|(idx, column)| {
                    let value = row.get_ref(idx)?;
                    Ok(RowField {
                        column_name: column.clone(),
                        value: format_value(value),
                        is_blob: matches!(value, rusqlite::types::ValueRef::Blob(_)),
                    })
                })
                .collect::<Result<Vec<_>, rusqlite::Error>>()?;
            return Ok(Some(RowRecord {
                rowid: None,
                row_label: format!("row {}", offset + 1),
                fields,
                foreign_keys: self.foreign_key_info(table_name)?,
            }));
        };
        let order_by = build_order_by_or_rowid(sort_clauses, rowid_alias);
        let (where_clause, mut filter_params) = build_filter_where(filter_clauses);
        let sql = format!(
            "SELECT {rowid_alias}, {select_list}
             FROM {safe_table_name}
             {where_clause}
             {order_by}
             LIMIT 1 OFFSET ?"
        );
        filter_params.push(Value::Integer(offset as i64));
        let mut stmt = match self.conn.prepare(&sql) {
            Ok(stmt) => stmt,
            Err(_) => {
                let fallback_sql = format!(
                    "SELECT {select_list}
                     FROM {safe_table_name}
                     {where_clause}
                     {}
                     LIMIT 1 OFFSET ?",
                    build_order_by(sort_clauses)
                );
                let mut fallback_stmt = self.conn.prepare(&fallback_sql)?;
                let mut rows = fallback_stmt.query(params_from_iter(filter_params.iter()))?;
                let Some(row) = rows.next()? else {
                    return Ok(None);
                };
                let fields = columns
                    .iter()
                    .enumerate()
                    .map(|(idx, column)| {
                        let value = row.get_ref(idx)?;
                        Ok(RowField {
                            column_name: column.clone(),
                            value: format_value(value),
                            is_blob: matches!(value, rusqlite::types::ValueRef::Blob(_)),
                        })
                    })
                    .collect::<Result<Vec<_>, rusqlite::Error>>()?;
                return Ok(Some(RowRecord {
                    rowid: None,
                    row_label: format!("row {}", offset + 1),
                    fields,
                    foreign_keys: self.foreign_key_info(table_name)?,
                }));
            }
        };

        let mut rows = stmt.query(params_from_iter(filter_params.iter()))?;
        let Some(row) = rows.next()? else {
            return Ok(None);
        };
        let rowid = row.get::<_, i64>(0)?;
        let fields = columns
            .iter()
            .enumerate()
            .map(|(idx, column)| {
                let value = row.get_ref(idx + 1)?;
                Ok(RowField {
                    column_name: column.clone(),
                    value: format_value(value),
                    is_blob: matches!(value, rusqlite::types::ValueRef::Blob(_)),
                })
            })
            .collect::<Result<Vec<_>, rusqlite::Error>>()?;

        Ok(Some(RowRecord {
            rowid: Some(rowid),
            row_label: format!("rowid {rowid}"),
            fields,
            foreign_keys: self.foreign_key_info(table_name)?,
        }))
    }

    pub fn locate_foreign_row_offset(
        &self,
        table_name: &str,
        column_name: &str,
        value: &str,
        sort_clauses: &[SortClause],
        filter_clauses: &[FilterClause],
    ) -> Result<Option<usize>> {
        let safe_table_name = quote_table_name(table_name);
        let safe_column_name = quote_identifier(column_name);
        let rowid_alias = self.rowid_alias(table_name)?;
        let order_by = build_window_order_by(sort_clauses, rowid_alias);
        let (where_clause, mut filter_params) = build_filter_where(filter_clauses);
        let sql = format!(
            "SELECT rn
             FROM (
                 SELECT ROW_NUMBER() OVER ({order_by}) - 1 AS rn, {safe_column_name} AS fk_value
                 FROM {safe_table_name}
                 {where_clause}
             )
             WHERE fk_value = ?
             LIMIT 1"
        );
        filter_params.push(Value::Text(value.to_string()));
        let result = self
            .conn
            .query_row(&sql, params_from_iter(filter_params.iter()), |row| {
                row.get::<_, i64>(0)
            });

        match result {
            Ok(offset) => Ok(Some(
                usize::try_from(offset).map_err(|_| anyhow!("row offset overflowed usize"))?,
            )),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(err) => Err(err.into()),
        }
    }

    pub fn locate_row_offset(
        &self,
        table_name: &str,
        rowid: i64,
        sort_clauses: &[SortClause],
        filter_clauses: &[FilterClause],
    ) -> Result<Option<usize>> {
        let safe_table_name = quote_table_name(table_name);
        let Some(rowid_alias) = self.rowid_alias(table_name)? else {
            return Ok(None);
        };
        let order_by = build_order_by_or_rowid(sort_clauses, rowid_alias);
        let (where_clause, mut filter_params) = build_filter_where(filter_clauses);
        let sql = format!(
            "SELECT rn
             FROM (
                 SELECT {rowid_alias}, ROW_NUMBER() OVER ({order_by}) - 1 AS rn
                 FROM {safe_table_name}
                 {where_clause}
             )
             WHERE {rowid_alias} = ?"
        );
        filter_params.push(Value::Integer(rowid));

        let result = self
            .conn
            .query_row(&sql, params_from_iter(filter_params.iter()), |row| {
                row.get::<_, i64>(0)
            });

        match result {
            Ok(offset) => Ok(Some(
                usize::try_from(offset).map_err(|_| anyhow!("row offset overflowed usize"))?,
            )),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(err) => Err(err.into()),
        }
    }
}

pub(crate) fn quote_identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('\"', "\"\""))
}

pub(crate) fn quote_table_name(value: &str) -> String {
    if let Some((schema, table)) = split_qualified_table_name(value) {
        format!("{}.{}", quote_identifier(schema), quote_identifier(table))
    } else {
        quote_identifier(value)
    }
}

pub(crate) fn split_qualified_table_name(value: &str) -> Option<(&str, &str)> {
    value.split_once('.')
}

pub(crate) fn build_filter_where(filter_clauses: &[FilterClause]) -> (String, Vec<Value>) {
    if filter_clauses.is_empty() {
        return (String::new(), Vec::new());
    }

    let mut parts = Vec::with_capacity(filter_clauses.len());
    let mut params = Vec::with_capacity(filter_clauses.len());

    for clause in filter_clauses {
        let safe_column = quote_identifier(&clause.column_name);
        match clause.mode {
            FilterMode::Contains => {
                parts.push(format!("{safe_column} LIKE ?"));
                params.push(Value::Text(format!("%{}%", clause.value)));
            }
            FilterMode::Equals => {
                parts.push(format!("{safe_column} = ?"));
                params.push(Value::Text(clause.value.clone()));
            }
            FilterMode::StartsWith => {
                parts.push(format!("{safe_column} LIKE ?"));
                params.push(Value::Text(format!("{}%", clause.value)));
            }
            FilterMode::GreaterThan => {
                parts.push(format!("CAST({safe_column} AS REAL) > CAST(? AS REAL)"));
                params.push(Value::Text(clause.value.clone()));
            }
            FilterMode::LessThan => {
                parts.push(format!("CAST({safe_column} AS REAL) < CAST(? AS REAL)"));
                params.push(Value::Text(clause.value.clone()));
            }
            FilterMode::IsTrue => parts.push(format!("CAST({safe_column} AS INTEGER) <> 0")),
            FilterMode::IsFalse => parts.push(format!("CAST({safe_column} AS INTEGER) = 0")),
        }
    }

    (format!(" WHERE {}", parts.join(" AND ")), params)
}

pub(crate) fn build_order_by(sort_clauses: &[SortClause]) -> String {
    if sort_clauses.is_empty() {
        String::new()
    } else {
        let clauses = sort_clauses
            .iter()
            .map(|clause| {
                let direction = if clause.descending { "DESC" } else { "ASC" };
                format!("{} {direction}", quote_identifier(&clause.column_name))
            })
            .collect::<Vec<_>>()
            .join(", ");
        format!(" ORDER BY {clauses}")
    }
}

pub(crate) fn build_order_by_or_rowid(sort_clauses: &[SortClause], rowid_alias: &str) -> String {
    build_window_order_by(sort_clauses, Some(rowid_alias))
}

impl Database {
    pub(crate) fn rowid_alias(&self, table_name: &str) -> Result<Option<&'static str>> {
        Ok(rowid_alias_from_columns(&self.column_info(table_name)?))
    }
}

pub(crate) fn build_window_order_by(
    sort_clauses: &[SortClause],
    rowid_alias: Option<&str>,
) -> String {
    if sort_clauses.is_empty() {
        rowid_alias
            .map(|rowid_alias| format!("ORDER BY {rowid_alias} ASC"))
            .unwrap_or_default()
    } else {
        build_order_by(sort_clauses).trim_start().to_string()
    }
}

pub(crate) fn rowid_alias_from_columns(columns: &[ColumnInfo]) -> Option<&'static str> {
    const CANDIDATES: [&str; 3] = ["_rowid_", "rowid", "oid"];

    if columns
        .iter()
        .filter(|column| column.is_primary_key)
        .count()
        == 1
        && let Some(column) = columns.iter().find(|column| {
            column.is_primary_key
                && column.data_type.eq_ignore_ascii_case("INTEGER")
                && CANDIDATES
                    .iter()
                    .any(|candidate| column.name.eq_ignore_ascii_case(candidate))
        })
    {
        return CANDIDATES
            .into_iter()
            .find(|candidate| column.name.eq_ignore_ascii_case(candidate));
    }

    CANDIDATES.into_iter().find(|candidate| {
        !columns
            .iter()
            .any(|column| column.name.eq_ignore_ascii_case(candidate))
    })
}

#[cfg(test)]
mod tests;
