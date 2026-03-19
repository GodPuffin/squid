use anyhow::{Result, anyhow};

use super::value::format_value;
use super::{Database, SqlExecutionResult};

impl Database {
    pub fn execute_sql(&self, sql: &str, row_limit: usize) -> Result<SqlExecutionResult> {
        let sql = sql.trim();
        if sql.is_empty() {
            return Err(anyhow!("query is empty"));
        }

        let mut stmt = self.conn.prepare(sql)?;
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
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use rusqlite::Connection;

    use super::super::SqlExecutionResult;
    use super::Database;

    #[test]
    fn execute_sql_returns_rows_for_select() {
        let path = temp_db_path("rows");
        let conn = Connection::open(&path).expect("create db");
        conn.execute("CREATE TABLE demo(id INTEGER PRIMARY KEY, name TEXT)", [])
            .expect("create table");
        conn.execute("INSERT INTO demo(name) VALUES ('alpha'), ('beta')", [])
            .expect("seed");
        drop(conn);

        let db = Database::open(&path).expect("open db");
        let result = db
            .execute_sql("SELECT name FROM demo ORDER BY id", 50)
            .expect("select");

        match result {
            SqlExecutionResult::Rows {
                columns,
                rows,
                is_mutation,
            } => {
                assert_eq!(columns, vec!["name"]);
                assert_eq!(
                    rows,
                    vec![vec!["alpha".to_string()], vec!["beta".to_string()]]
                );
                assert!(!is_mutation);
            }
            SqlExecutionResult::Statement { .. } => panic!("expected rows"),
        }

        let _ = fs::remove_file(path);
    }

    #[test]
    fn execute_sql_writes_changes() {
        let path = temp_db_path("write");
        let conn = Connection::open(&path).expect("create db");
        conn.execute("CREATE TABLE demo(id INTEGER PRIMARY KEY, name TEXT)", [])
            .expect("create table");
        drop(conn);

        let db = Database::open(&path).expect("open db");
        let result = db
            .execute_sql("INSERT INTO demo(name) VALUES ('gamma')", 50)
            .expect("insert");

        match result {
            SqlExecutionResult::Statement {
                affected_rows,
                description,
            } => {
                assert_eq!(affected_rows, 1);
                assert_eq!(description, "INSERT");
            }
            SqlExecutionResult::Rows { .. } => panic!("expected statement"),
        }

        let verify = Connection::open(&path).expect("reopen");
        let count = verify
            .query_row("SELECT COUNT(*) FROM demo", [], |row| row.get::<_, i64>(0))
            .expect("count");
        assert_eq!(count, 1);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn execute_sql_reports_errors() {
        let path = temp_db_path("error");
        let conn = Connection::open(&path).expect("create db");
        conn.execute("CREATE TABLE demo(id INTEGER PRIMARY KEY, name TEXT)", [])
            .expect("create table");
        drop(conn);

        let db = Database::open(&path).expect("open db");
        let error = db
            .execute_sql("SELECT missing FROM demo", 50)
            .expect_err("expected failure");

        assert!(error.to_string().contains("no such column"));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn execute_sql_marks_returning_mutations() {
        let path = temp_db_path("returning");
        let conn = Connection::open(&path).expect("create db");
        conn.execute("CREATE TABLE demo(id INTEGER PRIMARY KEY, name TEXT)", [])
            .expect("create table");
        drop(conn);

        let db = Database::open(&path).expect("open db");
        let result = db
            .execute_sql("INSERT INTO demo(name) VALUES ('delta') RETURNING id", 50)
            .expect("insert returning");

        match result {
            SqlExecutionResult::Rows {
                columns,
                rows,
                is_mutation,
            } => {
                assert_eq!(columns, vec!["id"]);
                assert_eq!(rows.len(), 1);
                assert!(is_mutation);
            }
            SqlExecutionResult::Statement { .. } => panic!("expected rows"),
        }

        let _ = fs::remove_file(path);
    }

    fn temp_db_path(label: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("squid-{label}-{stamp}.sqlite"))
    }
}
