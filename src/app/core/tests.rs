use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;

use super::App;

#[test]
fn refresh_loaded_db_state_preserves_selected_table_name() {
    let path = temp_db_path("refresh-selection");
    let conn = Connection::open(&path).expect("create db");
    conn.execute("CREATE TABLE users(id INTEGER PRIMARY KEY)", [])
        .expect("create users");
    drop(conn);

    let mut app = App::load(path.clone()).expect("load app");
    assert_eq!(app.selected_table_name(), Some("main.users"));

    app.db
        .as_ref()
        .expect("db loaded")
        .execute_sql("CREATE TABLE addresses(id INTEGER PRIMARY KEY)", 10)
        .expect("create addresses");
    app.refresh_loaded_db_state().expect("refresh app state");

    assert_eq!(app.selected_table_name(), Some("main.users"));

    let _ = fs::remove_file(path);
}

#[test]
fn request_quit_closes_search_before_exiting() {
    let path = temp_db_path("request-quit-search");
    let conn = Connection::open(&path).expect("create db");
    conn.execute("CREATE TABLE users(id INTEGER PRIMARY KEY)", [])
        .expect("create users");
    drop(conn);

    let mut app = App::load(path.clone()).expect("load app");
    app.open_search(crate::app::SearchScope::CurrentTable)
        .expect("open search");

    let should_quit = app.request_quit().expect("request quit");

    assert!(!should_quit);
    assert!(app.search.is_none());

    let _ = fs::remove_file(path);
}

#[test]
fn switching_to_browse_clears_sql_completion() {
    let path = temp_db_path("switch-browse-clears-completion");
    let conn = Connection::open(&path).expect("create db");
    conn.execute("CREATE TABLE users(id INTEGER PRIMARY KEY)", [])
        .expect("create users");
    drop(conn);

    let mut app = App::load(path.clone()).expect("load app");
    app.mode = crate::app::AppMode::Sql;
    app.sql.completion = Some(crate::app::SqlCompletionState {
        prefix_start: 0,
        items: vec![crate::app::SqlCompletionItem {
            label: "SELECT".to_string(),
            insert_text: "SELECT".to_string(),
        }],
        selected: 0,
    });

    app.handle(crate::app::Action::SwitchToBrowse)
        .expect("switch to browse");

    assert_eq!(app.mode, crate::app::AppMode::Browse);
    assert!(app.sql.completion.is_none());

    let _ = fs::remove_file(path);
}

#[test]
fn reload_preserves_connection_scoped_tables() {
    let path = temp_db_path("reload-preserves-temp");
    let conn = Connection::open(&path).expect("create db");
    conn.execute("CREATE TABLE users(id INTEGER PRIMARY KEY)", [])
        .expect("create users");
    drop(conn);

    let mut app = App::load(path.clone()).expect("load app");
    app.db
        .as_ref()
        .expect("db loaded")
        .execute_sql("CREATE TEMP TABLE scratch(value TEXT)", 10)
        .expect("create temp table");

    app.reload().expect("reload");

    assert!(
        app.tables.iter().any(|table| table.name == "temp.scratch"),
        "reload should keep connection-scoped temp tables visible"
    );

    let _ = fs::remove_file(path);
}

#[test]
fn open_database_clears_sql_column_cache() {
    let first = temp_db_path("open-clears-cache-first");
    let second = temp_db_path("open-clears-cache-second");

    let conn = Connection::open(&first).expect("create first db");
    conn.execute("CREATE TABLE users(old_column TEXT)", [])
        .expect("create first schema");
    drop(conn);

    let conn = Connection::open(&second).expect("create second db");
    conn.execute("CREATE TABLE users(new_column TEXT)", [])
        .expect("create second schema");
    drop(conn);

    let mut app = App::load(first.clone()).expect("load first app");
    app.sql.column_cache.insert(
        "main.users".to_string(),
        vec!["old_column".to_string()].into(),
    );

    app.open_database(&second).expect("open second db");
    assert!(app.sql.column_cache.is_empty());

    let _ = fs::remove_file(first);
    let _ = fs::remove_file(second);
}

fn temp_db_path(label: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("squid-core-{label}-{stamp}.sqlite"))
}
