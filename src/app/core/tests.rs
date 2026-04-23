use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;

use super::App;

#[test]
fn loading_without_path_starts_on_home_screen() {
    let app = App::load(None::<PathBuf>).expect("load app");

    assert!(app.is_home());
    assert!(app.path().is_none());
}

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
fn row_navigation_reuses_cached_row_count_until_reload() {
    let path = temp_db_path("navigation-reuses-count");
    let conn = Connection::open(&path).expect("create db");
    conn.execute("CREATE TABLE users(id INTEGER PRIMARY KEY, name TEXT)", [])
        .expect("create users");
    conn.execute("INSERT INTO users(name) VALUES ('alpha'), ('beta')", [])
        .expect("seed users");
    drop(conn);

    let mut app = App::load(path.clone()).expect("load app");
    app.set_viewport_sizes(1, 20, 40, 10)
        .expect("shrink viewport");
    assert_eq!(app.preview.total_rows, 2);

    app.db
        .as_ref()
        .expect("db loaded")
        .execute_sql("INSERT INTO users(name) VALUES ('gamma')", 10)
        .expect("insert row");

    app.move_row_selection_down().expect("move down one row");

    assert_eq!(app.preview.total_rows, 2);

    app.reload().expect("reload");

    assert_eq!(app.preview.total_rows, 3);

    let _ = fs::remove_file(path);
}

#[test]
fn row_navigation_recovers_when_cached_row_count_is_too_high() {
    let path = temp_db_path("navigation-shrink-count");
    let conn = Connection::open(&path).expect("create db");
    conn.execute("CREATE TABLE users(id INTEGER PRIMARY KEY, name TEXT)", [])
        .expect("create users");
    conn.execute("INSERT INTO users(name) VALUES ('alpha'), ('beta')", [])
        .expect("seed users");
    drop(conn);

    let mut app = App::load(path.clone()).expect("load app");
    app.set_viewport_sizes(1, 20, 40, 10)
        .expect("shrink viewport");
    assert_eq!(app.preview.total_rows, 2);

    app.db
        .as_ref()
        .expect("db loaded")
        .execute_sql("DELETE FROM users WHERE id = 2", 10)
        .expect("delete row");

    app.move_row_selection_down().expect("move down one row");

    assert_eq!(app.preview.total_rows, 1);
    assert_eq!(app.selected_row, 0);
    assert_eq!(app.row_offset, 0);

    let _ = fs::remove_file(path);
}

#[test]
fn refresh_preview_page_recovers_when_first_page_cached_row_count_is_too_high() {
    let path = temp_db_path("first-page-shrink-count");
    let conn = Connection::open(&path).expect("create db");
    conn.execute("CREATE TABLE users(id INTEGER PRIMARY KEY, name TEXT)", [])
        .expect("create users");
    conn.execute("INSERT INTO users(name) VALUES ('alpha')", [])
        .expect("seed users");
    drop(conn);

    let mut app = App::load(path.clone()).expect("load app");
    assert_eq!(app.preview.total_rows, 1);
    assert_eq!(app.preview.rows.len(), 1);

    app.db
        .as_ref()
        .expect("db loaded")
        .execute_sql("DELETE FROM users", 10)
        .expect("delete rows");

    app.refresh_preview_page().expect("refresh page");

    assert_eq!(app.preview.total_rows, 0);
    assert!(app.preview.rows.is_empty());
    assert_eq!(app.selected_row, 0);
    assert_eq!(app.row_offset, 0);

    let _ = fs::remove_file(path);
}

#[test]
fn refresh_preview_page_recounts_when_first_page_shrinks_but_is_not_empty() {
    let path = temp_db_path("first-page-partial-shrink-count");
    let conn = Connection::open(&path).expect("create db");
    conn.execute("CREATE TABLE users(id INTEGER PRIMARY KEY, name TEXT)", [])
        .expect("create users");
    conn.execute(
        "INSERT INTO users(name) VALUES ('alpha'), ('beta'), ('gamma')",
        [],
    )
    .expect("seed users");
    drop(conn);

    let mut app = App::load(path.clone()).expect("load app");
    app.set_viewport_sizes(10, 20, 40, 10)
        .expect("expand viewport");
    app.move_row_selection_down().expect("move down one row");
    app.move_row_selection_down().expect("move down two rows");
    assert_eq!(app.selected_row, 2);
    assert_eq!(app.preview.total_rows, 3);

    app.db
        .as_ref()
        .expect("db loaded")
        .execute_sql("DELETE FROM users WHERE id IN (2, 3)", 10)
        .expect("delete rows");

    app.refresh_preview_page().expect("refresh page");

    assert_eq!(app.preview.total_rows, 1);
    assert_eq!(app.preview.rows.len(), 1);
    assert_eq!(app.selected_row, 0);
    assert_eq!(app.row_offset, 0);
    assert_eq!(app.selected_row_in_view(), Some(0));

    let _ = fs::remove_file(path);
}

#[test]
fn refresh_preview_page_recounts_when_first_page_grows_past_cached_total() {
    let path = temp_db_path("first-page-grow-count");
    let conn = Connection::open(&path).expect("create db");
    conn.execute("CREATE TABLE users(id INTEGER PRIMARY KEY, name TEXT)", [])
        .expect("create users");
    conn.execute("INSERT INTO users(name) VALUES ('alpha')", [])
        .expect("seed users");
    drop(conn);

    let mut app = App::load(path.clone()).expect("load app");
    app.set_viewport_sizes(10, 20, 40, 10)
        .expect("expand viewport");
    assert_eq!(app.preview.total_rows, 1);
    assert_eq!(app.preview.rows.len(), 1);

    app.db
        .as_ref()
        .expect("db loaded")
        .execute_sql("INSERT INTO users(name) VALUES ('beta'), ('gamma')", 10)
        .expect("insert rows");

    app.refresh_preview_page().expect("refresh page");

    assert_eq!(app.preview.total_rows, 3);
    assert_eq!(app.preview.rows.len(), 3);

    app.move_row_selection_down().expect("move down one row");
    app.move_row_selection_down().expect("move down two rows");

    assert_eq!(app.selected_row, 2);
    assert_eq!(app.selected_row_in_view(), Some(2));

    let _ = fs::remove_file(path);
}

#[test]
fn jump_to_row_offset_recounts_before_fetching_target_page() {
    let path = temp_db_path("jump-recounts");
    let conn = Connection::open(&path).expect("create db");
    conn.execute("CREATE TABLE users(id INTEGER PRIMARY KEY, name TEXT)", [])
        .expect("create users");
    conn.execute("INSERT INTO users(name) VALUES ('alpha'), ('beta')", [])
        .expect("seed users");
    drop(conn);

    let mut app = App::load(path.clone()).expect("load app");
    app.set_viewport_sizes(1, 20, 40, 10)
        .expect("shrink viewport");
    assert_eq!(app.preview.total_rows, 2);

    app.db
        .as_ref()
        .expect("db loaded")
        .execute_sql("INSERT INTO users(name) VALUES ('gamma')", 10)
        .expect("insert row");

    app.jump_to_row_offset(2).expect("jump to third row");

    assert_eq!(app.preview.total_rows, 3);
    assert_eq!(app.selected_row, 2);
    assert_eq!(app.row_offset, 2);

    let _ = fs::remove_file(path);
}

#[test]
fn reload_clears_cached_table_metadata() {
    let path = temp_db_path("reload-clears-metadata");
    let conn = Connection::open(&path).expect("create db");
    conn.execute("CREATE TABLE users(id INTEGER PRIMARY KEY)", [])
        .expect("create users");
    drop(conn);

    let mut app = App::load(path.clone()).expect("load app");
    let columns = app
        .details
        .as_ref()
        .expect("details loaded")
        .columns
        .iter()
        .map(|column| column.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(columns, vec!["id"]);

    let conn = Connection::open(&path).expect("reopen db");
    conn.execute("ALTER TABLE users ADD COLUMN email TEXT", [])
        .expect("alter users");
    drop(conn);

    app.reload().expect("reload");

    let columns = app
        .details
        .as_ref()
        .expect("details reloaded")
        .columns
        .iter()
        .map(|column| column.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(columns, vec!["id", "email"]);

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
