use anyhow::{Result, anyhow};

use super::value::format_value;
use super::{Database, SqlExecutionResult};

impl Database {
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
            let rows = stmt
                .query_map([], |row| {
                    let mut values = Vec::with_capacity(column_count);
                    for idx in 0..column_count {
                        values.push(format_value(row.get_ref(idx)?));
                    }
                    Ok(values)
                })?
                .take(row_limit)
                .collect::<Result<Vec<_>, _>>()?;

            Ok(SqlExecutionResult::Rows {
                columns,
                rows,
                is_mutation,
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
#[path = "../testing/db/execute.rs"]
mod tests;
