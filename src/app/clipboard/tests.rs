use rusqlite::Connection;

use super::super::{App, AppMode, ContentView, PaneFocus, SqlPane};

#[test]
fn preview_cell_copy_text_returns_selected_cell_value() {
    let path = temp_db_path("clipboard-preview");
    let conn = Connection::open(&path).expect("create db");
    conn.execute("CREATE TABLE t(id INTEGER PRIMARY KEY, name TEXT)", [])
        .expect("create table");
    conn.execute("INSERT INTO t(name) VALUES ('alpha'), ('beta')", [])
        .expect("insert rows");
    drop(conn);

    let mut app = App::load(path.clone()).expect("load app");
    app.focus = PaneFocus::Content;
    app.content_view = ContentView::Rows;
    app.selected_column = 1;
    assert_eq!(app.preview_cell_copy_text().as_deref(), Some("alpha"));

    let _ = std::fs::remove_file(path);
}

#[test]
fn sql_copy_text_returns_full_query() {
    let path = temp_db_path("clipboard-sql");
    let conn = Connection::open(&path).expect("create db");
    conn.execute("CREATE TABLE t(id INTEGER PRIMARY KEY)", [])
        .expect("create table");
    drop(conn);

    let mut app = App::load(path.clone()).expect("load app");
    app.mode = AppMode::Sql;
    app.sql.focus = SqlPane::Editor;
    app.sql.query = "SELECT 1".to_string();

    assert_eq!(app.sql_copy_text().as_deref(), Some("SELECT 1"));

    let _ = std::fs::remove_file(path);
}

fn temp_db_path(label: &str) -> std::path::PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("squid-clipboard-{label}-{stamp}.sqlite"))
}
