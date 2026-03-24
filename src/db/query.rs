use anyhow::{Result, anyhow};
use rusqlite::params_from_iter;
use rusqlite::types::Value;

use super::schema::count_rows;
use super::value::format_value;
use super::{Database, FilterClause, FilterMode, RowPreview, RowRecord, SortClause};

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
        let sql = format!(
            "SELECT rowid, {select_list} FROM {safe_table_name}{where_clause}{order_by} LIMIT ? OFFSET ?"
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
                    Ok((None, values))
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
        let columns = self.list_columns(table_name)?;
        if columns.is_empty() {
            return Ok(None);
        }

        let safe_table_name = quote_table_name(table_name);
        let select_list = columns
            .iter()
            .map(|column| quote_identifier(column))
            .collect::<Vec<_>>()
            .join(", ");
        let order_by = build_order_by_or_rowid(sort_clauses);
        let (where_clause, mut filter_params) = build_filter_where(filter_clauses);
        let sql = format!(
            "SELECT rowid, {select_list}
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
                    .map(|(idx, column)| Ok((column.clone(), format_value(row.get_ref(idx)?))))
                    .collect::<Result<Vec<_>, rusqlite::Error>>()?;
                return Ok(Some(RowRecord {
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
            .map(|(idx, column)| Ok((column.clone(), format_value(row.get_ref(idx + 1)?))))
            .collect::<Result<Vec<_>, rusqlite::Error>>()?;

        Ok(Some(RowRecord {
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
        let order_by = build_order_by_or_rowid(sort_clauses);
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
        let order_by = build_order_by_or_rowid(sort_clauses);
        let (where_clause, mut filter_params) = build_filter_where(filter_clauses);
        let sql = format!(
            "SELECT rn
             FROM (
                 SELECT rowid, ROW_NUMBER() OVER ({order_by}) - 1 AS rn
                 FROM {safe_table_name}
                 {where_clause}
             )
             WHERE rowid = ?"
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

pub(crate) fn build_order_by_or_rowid(sort_clauses: &[SortClause]) -> String {
    if sort_clauses.is_empty() {
        "ORDER BY rowid ASC".to_string()
    } else {
        build_order_by(sort_clauses).trim_start().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::{FilterClause, FilterMode, SortClause, build_filter_where, build_order_by};
    use rusqlite::types::Value;

    #[test]
    fn build_filter_where_uses_all_clauses() {
        let clauses = vec![
            FilterClause {
                column_name: "name".into(),
                mode: FilterMode::Contains,
                value: "sam".into(),
            },
            FilterClause {
                column_name: "active".into(),
                mode: FilterMode::IsTrue,
                value: String::new(),
            },
        ];

        let (sql, params) = build_filter_where(&clauses);
        assert!(sql.contains("\"name\" LIKE ?"));
        assert!(sql.contains("CAST(\"active\" AS INTEGER) <> 0"));
        assert_eq!(params, vec![Value::Text("%sam%".into())]);
    }

    #[test]
    fn build_order_by_keeps_sort_priority() {
        let clauses = vec![
            SortClause {
                column_name: "last_name".into(),
                descending: false,
            },
            SortClause {
                column_name: "created_at".into(),
                descending: true,
            },
        ];

        assert_eq!(
            build_order_by(&clauses),
            " ORDER BY \"last_name\" ASC, \"created_at\" DESC"
        );
    }
}
