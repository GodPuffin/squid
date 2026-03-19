use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;

use super::{completion_prefix, completion_qualifier, line_col_from_index, move_vertical};
use crate::app::App;

#[test]
fn completion_prefix_reads_identifier_prefix() {
    let query = "SELECT ac";
    let (start, prefix) = completion_prefix(query, query.len());
    assert_eq!(start, 7);
    assert_eq!(prefix, "ac");
}

#[test]
fn line_column_round_trips() {
    let query = "SELECT\nname";
    assert_eq!(line_col_from_index(query, 0), (0, 0));
    assert_eq!(line_col_from_index(query, 7), (1, 0));
}

#[test]
fn vertical_movement_preserves_column_when_possible() {
    let query = "SELECT\ncolumn\nx";
    let moved = move_vertical(query, query.len() - 1, -1);
    assert_eq!(line_col_from_index(query, moved), (1, 0));
}

#[test]
fn completion_qualifier_keeps_table_or_alias_prefix() {
    assert_eq!(completion_qualifier("orders."), "orders.");
    assert_eq!(completion_qualifier("o.id"), "o.");
    assert_eq!(completion_qualifier("id"), "");
}

#[test]
fn sql_completion_preserves_qualified_prefix_when_applied() {
    let path = temp_db_path("qualified-completion");
    let conn = Connection::open(&path).expect("create db");
    conn.execute("CREATE TABLE orders(id INTEGER PRIMARY KEY, name TEXT)", [])
        .expect("create table");
    drop(conn);

    let mut app = App::load(Some(path.clone())).expect("load app");
    app.sql.query = "SELECT orders.".to_string();
    app.sql.cursor = app.sql.query.len();
    app.sql_refresh_completion().expect("refresh completion");
    let completion = app.sql.completion.as_mut().expect("completion");
    completion.selected = completion
        .items
        .iter()
        .position(|item| item.label == "orders.id")
        .expect("orders.id completion");

    app.sql_apply_completion();

    assert_eq!(app.sql.query, "SELECT orders.id");

    let _ = fs::remove_file(path);
}

#[test]
fn sql_completion_matches_alias_qualified_prefixes() {
    let path = temp_db_path("alias-completion");
    let conn = Connection::open(&path).expect("create db");
    conn.execute("CREATE TABLE orders(id INTEGER PRIMARY KEY, name TEXT)", [])
        .expect("create table");
    drop(conn);

    let mut app = App::load(Some(path.clone())).expect("load app");
    app.sql.query = "SELECT o.".to_string();
    app.sql.cursor = app.sql.query.len();
    app.sql_refresh_completion().expect("refresh completion");

    let items = app
        .sql
        .completion
        .as_ref()
        .expect("completion")
        .items
        .iter()
        .map(|item| item.insert_text.as_str())
        .collect::<Vec<_>>();

    assert!(items.contains(&"o.id"));
    assert!(items.contains(&"o.name"));

    let _ = fs::remove_file(path);
}

#[test]
fn sql_completion_matches_unqualified_column_prefixes() {
    let path = temp_db_path("column-prefix-completion");
    let conn = Connection::open(&path).expect("create db");
    conn.execute("CREATE TABLE orders(id INTEGER PRIMARY KEY, name TEXT)", [])
        .expect("create table");
    drop(conn);

    let mut app = App::load(Some(path.clone())).expect("load app");
    app.sql.query = "SELECT na".to_string();
    app.sql.cursor = app.sql.query.len();
    app.sql_refresh_completion().expect("refresh completion");

    let items = app
        .sql
        .completion
        .as_ref()
        .expect("completion")
        .items
        .iter()
        .map(|item| item.insert_text.as_str())
        .collect::<Vec<_>>();

    assert!(items.contains(&"name"));

    let _ = fs::remove_file(path);
}

#[test]
fn sql_execute_reloads_after_insert_returning() {
    let path = temp_db_path("insert-returning");
    let conn = Connection::open(&path).expect("create db");
    conn.execute("CREATE TABLE demo(id INTEGER PRIMARY KEY, name TEXT)", [])
        .expect("create table");
    drop(conn);

    let mut app = App::load(Some(path.clone())).expect("load app");
    assert_eq!(app.preview.total_rows, 0);

    app.sql.query = "INSERT INTO demo(name) VALUES ('delta') RETURNING id".to_string();
    app.sql.cursor = app.sql.query.len();
    app.sql_execute().expect("execute sql");

    assert_eq!(app.preview.total_rows, 1);

    let _ = fs::remove_file(path);
}

#[test]
fn sql_execute_preserves_connection_scoped_state() {
    let path = temp_db_path("connection-state");
    let conn = Connection::open(&path).expect("create db");
    conn.execute("CREATE TABLE demo(id INTEGER PRIMARY KEY, name TEXT)", [])
        .expect("create table");
    drop(conn);

    let mut app = App::load(Some(path.clone())).expect("load app");

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

    let mut app = App::load(Some(path.clone())).expect("load app");

    app.sql.query = "CREATE TEMP TABLE temp_demo(value TEXT)".to_string();
    app.sql.cursor = app.sql.query.len();
    app.sql_execute().expect("create temp table");

    let temp_index = app
        .tables
        .iter()
        .position(|table| table.name == "temp_demo")
        .expect("temp table should be listed");
    app.selected_table = temp_index;
    app.refresh_preview().expect("refresh temp preview");

    assert_eq!(app.selected_table_name(), Some("temp_demo"));
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
