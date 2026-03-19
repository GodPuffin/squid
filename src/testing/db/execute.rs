use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;

use super::super::SqlExecutionResult;
use super::{Database, describe_statement};

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
        } => {
            assert_eq!(columns, vec!["name"]);
            assert_eq!(rows, vec![vec!["alpha".to_string()]]);
            assert!(!is_mutation);
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
        } => {
            assert_eq!(columns, vec!["id"]);
            assert_eq!(rows.len(), 1);
            assert!(is_mutation);
        }
        SqlExecutionResult::Statement { .. } => panic!("expected rows"),
    }

    let _ = fs::remove_file(path);
}

#[test]
fn execute_sql_rejects_empty_and_whitespace_only_queries() {
    let path = temp_db_path("empty");
    let conn = Connection::open(&path).expect("create db");
    conn.execute("CREATE TABLE demo(id INTEGER PRIMARY KEY, name TEXT)", [])
        .expect("create table");
    drop(conn);

    let db = Database::open(&path).expect("open db");
    for query in ["", "   \n\t  "] {
        let error = db.execute_sql(query, 50).expect_err("expected empty query failure");
        assert!(error.to_string().contains("query is empty"));
    }

    let _ = fs::remove_file(path);
}

#[test]
fn execute_sql_respects_row_limit() {
    let path = temp_db_path("limit");
    let conn = Connection::open(&path).expect("create db");
    conn.execute("CREATE TABLE demo(id INTEGER PRIMARY KEY, name TEXT)", [])
        .expect("create table");
    conn.execute(
        "INSERT INTO demo(name) VALUES ('alpha'), ('beta'), ('gamma')",
        [],
    )
    .expect("seed");
    drop(conn);

    let db = Database::open(&path).expect("open db");
    let result = db
        .execute_sql("SELECT name FROM demo ORDER BY id", 2)
        .expect("select");

    match result {
        SqlExecutionResult::Rows { rows, .. } => assert_eq!(rows.len(), 2),
        SqlExecutionResult::Statement { .. } => panic!("expected rows"),
    }

    let _ = fs::remove_file(path);
}

#[test]
fn execute_sql_describes_update_statements() {
    let path = temp_db_path("update");
    let conn = Connection::open(&path).expect("create db");
    conn.execute("CREATE TABLE demo(id INTEGER PRIMARY KEY, name TEXT)", [])
        .expect("create table");
    conn.execute("INSERT INTO demo(name) VALUES ('alpha')", [])
        .expect("seed");
    drop(conn);

    let db = Database::open(&path).expect("open db");
    let result = db
        .execute_sql("UPDATE demo SET name = 'beta' WHERE id = 1", 50)
        .expect("update");

    match result {
        SqlExecutionResult::Statement { description, .. } => assert_eq!(description, "UPDATE"),
        SqlExecutionResult::Rows { .. } => panic!("expected statement"),
    }

    let _ = fs::remove_file(path);
}

#[test]
fn describe_statement_falls_back_for_blank_input() {
    assert_eq!(describe_statement(" \n\t "), "STATEMENT");
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
