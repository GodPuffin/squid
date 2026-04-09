use anyhow::{Result, anyhow, bail};
use rusqlite::params_from_iter;
use rusqlite::types::Value;

use super::query::{quote_identifier, quote_table_name};
use super::value::format_value;
use super::{Database, SqlExecutionResult};

impl Database {
    pub fn update_row_values(
        &self,
        table_name: &str,
        rowid: i64,
        changes: &[(String, Value)],
    ) -> Result<i64> {
        if changes.is_empty() {
            return Ok(rowid);
        }

        let rowid_column = "_rowid_";
        let assignments = changes
            .iter()
            .map(|(column_name, _)| format!("{} = ?", quote_identifier(column_name)))
            .collect::<Vec<_>>()
            .join(", ");
        let table_name = quote_table_name(table_name);

        let sql = format!(
            "UPDATE {table_name} SET {assignments} WHERE {rowid_column} = ? RETURNING {rowid_column}"
        );

        let mut params = changes
            .iter()
            .map(|(_, value)| value.clone())
            .collect::<Vec<_>>();
        params.push(Value::Integer(rowid));

        let mut stmt = self.conn.prepare(&sql)?;
        let mut rows = stmt.query(params_from_iter(params.iter()))?;
        let Some(row) = rows.next()? else {
            bail!("refusing to update: expected exactly one updated row, updated 0");
        };
        let updated_rowid = row.get::<_, i64>(0)?;

        if rows.next()?.is_some() {
            bail!("refusing to update: expected exactly one updated row, updated multiple");
        }

        Ok(updated_rowid)
    }

    pub fn execute_sql(&self, sql: &str, row_limit: usize) -> Result<SqlExecutionResult> {
        let sql = sql.trim();
        if sql.is_empty() {
            return Err(anyhow!("query is empty"));
        }

        let mut stmt = self
            .conn
            .prepare(sql)
            .map_err(|err| anyhow!("failed to prepare SQL: {err}"))?;
        let is_mutation = !stmt.readonly();
        if stmt.column_count() > 0 {
            let columns = stmt
                .column_names()
                .into_iter()
                .map(str::to_string)
                .collect::<Vec<_>>();
            let column_count = columns.len();
            let mut rows = stmt
                .query_map([], |row| {
                    let mut values = Vec::with_capacity(column_count);
                    for idx in 0..column_count {
                        values.push(format_value(row.get_ref(idx)?));
                    }
                    Ok(values)
                })?
                .take(row_limit + 1)
                .collect::<Result<Vec<_>, _>>()?;
            let is_truncated = rows.len() > row_limit;
            if is_truncated {
                rows.truncate(row_limit);
            }

            Ok(SqlExecutionResult::Rows {
                columns,
                rows,
                is_mutation,
                is_truncated,
            })
        } else {
            drop(stmt);
            let affected_rows = self.conn.execute(sql, [])?;
            Ok(SqlExecutionResult::Statement {
                affected_rows,
                description: describe_statement(sql),
            })
        }
    }
}

fn describe_statement(sql: &str) -> String {
    sql.split_whitespace()
        .next()
        .map(|keyword| keyword.to_uppercase())
        .filter(|keyword| !keyword.is_empty())
        .unwrap_or_else(|| "STATEMENT".to_string())
}

#[cfg(test)]
mod tests;
