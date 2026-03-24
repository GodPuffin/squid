use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;

use super::sql_rows_summary;
use crate::app::App;

#[test]
fn sql_execute_reloads_after_insert_returning() {
    let path = temp_db_path("insert-returning");
    let conn = Connection::open(&path).expect("create db");
    conn.execute("CREATE TABLE demo(id INTEGER PRIMARY KEY, name TEXT)", [])
        .expect("create table");
    drop(conn);

    let mut app = App::load(path.clone()).expect("load app");
    assert_eq!(app.preview.total_rows, 0);

    app.sql.query = "INSERT INTO demo(name) VALUES ('delta') RETURNING id".to_string();
    app.sql.cursor = app.sql.query.len();
    app.sql_execute().expect("execute sql");

    assert_eq!(app.preview.total_rows, 1);

    let _ = fs::remove_file(path);
}

#[test]
fn sql_rows_summary_marks_truncation() {
    assert_eq!(sql_rows_summary(200, false), "Returned 200 row(s)");
    assert_eq!(
        sql_rows_summary(200, true),
        "Returned 200 row(s) (truncated at 200)"
    );
}

#[test]
fn sql_execute_preserves_connection_scoped_state() {
    let path = temp_db_path("connection-state");
    let conn = Connection::open(&path).expect("create db");
    conn.execute("CREATE TABLE demo(id INTEGER PRIMARY KEY, name TEXT)", [])
        .expect("create table");
    drop(conn);

    let mut app = App::load(path.clone()).expect("load app");

    app.sql.query = "CREATE TEMP TABLE temp_demo(value TEXT)".to_string();
    app.sql.cursor = app.sql.query.len();
    app.sql_execute().expect("create temp table");

    app.sql.query = "INSERT INTO temp_demo(value) VALUES ('kept')".to_string();
    app.sql.cursor = app.sql.query.len();
    app.sql_execute().expect("insert temp row");

    app.sql.query = "SELECT value FROM temp_demo".to_string();
    app.sql.cursor = app.sql.query.len();
    app.sql_execute().expect("select temp row");

    match &app.sql.result {
        crate::app::SqlResultState::Rows { columns, rows } => {
            assert_eq!(columns, &vec!["value".to_string()]);
            assert_eq!(rows, &vec![vec!["kept".to_string()]]);
        }
        result => panic!("expected rows, got {result:?}"),
    }

    let _ = fs::remove_file(path);
}

#[test]
fn sql_execute_keeps_temp_tables_visible_in_browse() {
    let path = temp_db_path("temp-browse");
    let conn = Connection::open(&path).expect("create db");
    conn.execute("CREATE TABLE demo(id INTEGER PRIMARY KEY, name TEXT)", [])
        .expect("create table");
    drop(conn);

    let mut app = App::load(path.clone()).expect("load app");

    app.sql.query = "CREATE TEMP TABLE temp_demo(value TEXT)".to_string();
    app.sql.cursor = app.sql.query.len();
    app.sql_execute().expect("create temp table");

    let temp_index = app
        .tables
        .iter()
        .position(|table| table.name == "temp.temp_demo")
        .expect("temp table should be listed");
    app.selected_table = temp_index;
    app.refresh_preview().expect("refresh temp preview");

    assert_eq!(app.selected_table_name(), Some("temp.temp_demo"));
    assert_eq!(app.preview.total_rows, 0);
    assert_eq!(
        app.details
            .as_ref()
            .and_then(|details| details.create_sql.as_deref()),
        Some("CREATE TABLE temp_demo(value TEXT)")
    );

    let _ = fs::remove_file(path);
}

fn temp_db_path(label: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("squid-sql-{label}-{stamp}.sqlite"))
}
