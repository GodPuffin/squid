use std::fs;
use std::path::{Path, PathBuf};
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
            is_truncated,
        } => {
            assert_eq!(columns, vec!["name"]);
            assert_eq!(
                rows,
                vec![vec!["alpha".to_string()], vec!["beta".to_string()]]
            );
            assert!(!is_mutation);
            assert!(!is_truncated);
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
fn open_read_only_database_allows_selects() {
    let path = temp_db_path("readonly-open");
    let conn = Connection::open(&path).expect("create db");
    conn.execute("CREATE TABLE demo(id INTEGER PRIMARY KEY, name TEXT)", [])
        .expect("create table");
    conn.execute("INSERT INTO demo(name) VALUES ('alpha')", [])
        .expect("seed");
    drop(conn);

    let uri = read_only_uri(&path);
    let db = Database::open(uri.as_path()).expect("open db");
    let result = db
        .execute_sql("SELECT name FROM demo", 50)
        .expect("select from readonly db");

    match result {
        SqlExecutionResult::Rows {
            columns,
            rows,
            is_mutation,
            is_truncated,
        } => {
            assert_eq!(columns, vec!["name"]);
            assert_eq!(rows, vec![vec!["alpha".to_string()]]);
            assert!(!is_mutation);
            assert!(!is_truncated);
        }
        SqlExecutionResult::Statement { .. } => panic!("expected rows"),
    }

    let _ = fs::remove_file(path);
}

#[test]
fn execute_sql_reports_read_only_write_failures() {
    let path = temp_db_path("readonly-write");
    let conn = Connection::open(&path).expect("create db");
    conn.execute("CREATE TABLE demo(id INTEGER PRIMARY KEY, name TEXT)", [])
        .expect("create table");
    drop(conn);

    let uri = read_only_uri(&path);
    let db = Database::open(uri.as_path()).expect("open db");
    let error = db
        .execute_sql("INSERT INTO demo(name) VALUES ('blocked')", 50)
        .expect_err("write should fail");

    assert!(error.to_string().to_ascii_lowercase().contains("readonly"));

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
fn execute_sql_rejects_multiple_statements() {
    let path = temp_db_path("multi");
    let conn = Connection::open(&path).expect("create db");
    conn.execute("CREATE TABLE demo(id INTEGER PRIMARY KEY, name TEXT)", [])
        .expect("create table");
    drop(conn);

    let db = Database::open(&path).expect("open db");
    let error = db
        .execute_sql("SELECT 1; SELECT 2", 50)
        .expect_err("expected multiple statement failure");

    assert!(error.to_string().to_ascii_lowercase().contains("multiple"));

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
            is_truncated,
        } => {
            assert_eq!(columns, vec!["id"]);
            assert_eq!(rows.len(), 1);
            assert!(is_mutation);
            assert!(!is_truncated);
        }
        SqlExecutionResult::Statement { .. } => panic!("expected rows"),
    }

    let _ = fs::remove_file(path);
}

#[test]
fn execute_sql_marks_truncated_row_results() {
    let path = temp_db_path("truncated");
    let conn = Connection::open(&path).expect("create db");
    conn.execute("CREATE TABLE demo(id INTEGER PRIMARY KEY, name TEXT)", [])
        .expect("create table");
    for idx in 0..205 {
        conn.execute(
            "INSERT INTO demo(name) VALUES (?1)",
            [format!("name-{idx}")],
        )
        .expect("insert row");
    }
    drop(conn);

    let db = Database::open(&path).expect("open db");
    let result = db
        .execute_sql("SELECT name FROM demo ORDER BY id", 200)
        .expect("select");

    match result {
        SqlExecutionResult::Rows {
            rows, is_truncated, ..
        } => {
            assert_eq!(rows.len(), 200);
            assert!(is_truncated);
        }
        SqlExecutionResult::Statement { .. } => panic!("expected rows"),
    }

    let _ = fs::remove_file(path);
}

#[test]
fn update_row_values_rejects_ambiguous_rowid_predicates() {
    let path = temp_db_path("row-update-reserved");
    let conn = Connection::open(&path).expect("create db");
    conn.execute("CREATE TABLE demo(_rowid_ INTEGER, name TEXT)", [])
        .expect("create table");
    conn.execute("INSERT INTO demo(_rowid_, name) VALUES (7, 'first')", [])
        .expect("insert first");
    conn.execute("INSERT INTO demo(_rowid_, name) VALUES (7, 'second')", [])
        .expect("insert second");
    drop(conn);

    let db = Database::open(&path).expect("open db");
    let err = db
        .update_row_values(
            "demo",
            1,
            &[(
                "name".to_string(),
                rusqlite::types::Value::Text("updated".to_string()),
            )],
        )
        .expect_err("multi-row updates should be rejected");

    assert!(
        err.to_string().contains("expected exactly one updated row"),
        "{err}"
    );

    let _ = fs::remove_file(path);
}

#[test]
fn update_row_values_rejects_missing_rowid() {
    let path = temp_db_path("row-update-missing");
    let conn = Connection::open(&path).expect("create db");
    conn.execute("CREATE TABLE demo(id INTEGER PRIMARY KEY, name TEXT)", [])
        .expect("create table");
    conn.execute("INSERT INTO demo(name) VALUES ('alpha')", [])
        .expect("insert");
    drop(conn);

    let db = Database::open(&path).expect("open db");
    let err = db
        .update_row_values(
            "demo",
            999,
            &[(
                "name".to_string(),
                rusqlite::types::Value::Text("updated".to_string()),
            )],
        )
        .expect_err("missing row should be rejected");

    assert!(
        err.to_string().contains("expected exactly one updated row"),
        "{err}"
    );

    let _ = fs::remove_file(path);
}

#[test]
fn update_row_values_updates_matching_hidden_rowid() {
    let path = temp_db_path("row-update-hidden");
    let conn = Connection::open(&path).expect("create db");
    conn.execute("CREATE TABLE demo(id INTEGER PRIMARY KEY, name TEXT)", [])
        .expect("create table");
    conn.execute("INSERT INTO demo(name) VALUES ('alpha'), ('beta')", [])
        .expect("seed");
    drop(conn);

    let db = Database::open(&path).expect("open db");
    let updated_rows = db
        .update_row_values(
            "demo",
            1,
            &[(
                "name".to_string(),
                rusqlite::types::Value::Text("updated".to_string()),
            )],
        )
        .expect("update should succeed");

    assert_eq!(updated_rows, 1);

    let verify = Connection::open(&path).expect("reopen");
    let values = verify
        .prepare("SELECT name FROM demo ORDER BY id")
        .expect("prepare")
        .query_map([], |row| row.get::<_, String>(0))
        .expect("query")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect");
    assert_eq!(values, vec!["updated".to_string(), "beta".to_string()]);

    let _ = fs::remove_file(path);
}

#[test]
fn update_row_values_supports_schema_qualified_table_names() {
    let path = temp_db_path("row-update-qualified");
    let conn = Connection::open(&path).expect("create db");
    conn.execute("CREATE TABLE demo(id INTEGER PRIMARY KEY, name TEXT)", [])
        .expect("create table");
    conn.execute("INSERT INTO demo(name) VALUES ('alpha')", [])
        .expect("seed");
    drop(conn);

    let db = Database::open(&path).expect("open db");
    let updated_rows = db
        .update_row_values(
            "main.demo",
            1,
            &[(
                "name".to_string(),
                rusqlite::types::Value::Text("updated".to_string()),
            )],
        )
        .expect("qualified update should succeed");
    assert_eq!(updated_rows, 1);

    let verify = Connection::open(&path).expect("reopen");
    let value = verify
        .query_row("SELECT name FROM demo WHERE id = 1", [], |row| {
            row.get::<_, String>(0)
        })
        .expect("select");
    assert_eq!(value, "updated");

    let _ = fs::remove_file(path);
}

fn read_only_uri(path: &Path) -> PathBuf {
    PathBuf::from(format!("file:{}?mode=ro", path.display()))
}

fn temp_db_path(label: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("squid-{label}-{stamp}.sqlite"))
}
