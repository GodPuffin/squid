use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;

use super::Database;

#[test]
fn list_tables_includes_attached_schemas() {
    let main_path = temp_db_path("main");
    let attached_path = temp_db_path("attached");

    let conn = Connection::open(&main_path).expect("create main db");
    conn.execute("CREATE TABLE main_only(id INTEGER PRIMARY KEY)", [])
        .expect("create main table");
    conn.execute(
        "ATTACH DATABASE ?1 AS other",
        [attached_path.to_string_lossy().into_owned()],
    )
    .expect("attach db");
    conn.execute("CREATE TABLE other.other_only(id INTEGER PRIMARY KEY)", [])
        .expect("create attached table");
    drop(conn);

    let db = Database::open(&main_path).expect("open db");
    db.conn
        .execute(
            "ATTACH DATABASE ?1 AS other",
            [attached_path.to_string_lossy().into_owned()],
        )
        .expect("attach db on app connection");
    let tables = db.list_tables().expect("list tables");
    let names = tables
        .into_iter()
        .map(|table| table.name)
        .collect::<Vec<_>>();

    assert!(names.contains(&"main.main_only".to_string()));
    assert!(names.contains(&"other.other_only".to_string()));

    let _ = fs::remove_file(main_path);
    let _ = fs::remove_file(attached_path);
}

#[test]
fn table_writability_is_reported_per_attached_schema() {
    let main_path = temp_db_path("writable-main");
    let attached_path = temp_db_path("readonly-attached");

    let conn = Connection::open(&main_path).expect("create main db");
    conn.execute("CREATE TABLE main_only(id INTEGER PRIMARY KEY)", [])
        .expect("create main table");
    conn.execute("CREATE TABLE demo(id INTEGER PRIMARY KEY)", [])
        .expect("create main demo");
    drop(conn);

    let conn = Connection::open(&attached_path).expect("create attached db");
    conn.execute("CREATE TABLE demo(id INTEGER PRIMARY KEY)", [])
        .expect("create attached demo");
    drop(conn);

    let db = Database::open(&main_path).expect("open db");
    db.conn
        .execute(
            "ATTACH DATABASE ?1 AS other",
            [format!("file:{}?mode=ro", attached_path.display())],
        )
        .expect("attach read-only db on app connection");

    assert!(db.table_is_writable("main.demo").expect("main writable"));
    assert!(!db.table_is_writable("other.demo").expect("other read-only"));

    let _ = fs::remove_file(main_path);
    let _ = fs::remove_file(attached_path);
}

fn temp_db_path(label: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("squid-db-{label}-{stamp}.sqlite"))
}
