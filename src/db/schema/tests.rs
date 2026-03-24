use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;

use super::Database;

#[test]
fn foreign_key_info_preserves_schema_on_targets() {
    let path = temp_db_path("fk-schema");
    let conn = Connection::open(&path).expect("create db");
    conn.execute_batch(
        "CREATE TABLE customers(id INTEGER PRIMARY KEY, name TEXT);
         CREATE TABLE orders(
             id INTEGER PRIMARY KEY,
             customer_id INTEGER NOT NULL REFERENCES customers(id)
         );",
    )
    .expect("create schema");
    drop(conn);

    let db = Database::open(&path).expect("open db");
    let foreign_keys = db.foreign_key_info("main.orders").expect("foreign keys");

    assert_eq!(foreign_keys.len(), 1);
    assert_eq!(foreign_keys[0].target_table, "main.customers");

    let _ = fs::remove_file(path);
}

#[test]
fn table_details_reads_create_sql_from_attached_schema() {
    let main_path = temp_db_path("attached-main");
    let attached_path = temp_db_path("attached-other");

    let conn = Connection::open(&main_path).expect("create main db");
    conn.execute(
        "ATTACH DATABASE ?1 AS other",
        [attached_path.to_string_lossy().into_owned()],
    )
    .expect("attach db");
    conn.execute("CREATE TABLE other.demo(id INTEGER PRIMARY KEY)", [])
        .expect("create attached table");
    drop(conn);

    let db = Database::open(&main_path).expect("open db");
    db.conn
        .execute(
            "ATTACH DATABASE ?1 AS other",
            [attached_path.to_string_lossy().into_owned()],
        )
        .expect("attach db");
    let details = db
        .table_details("other.demo")
        .expect("attached table details");

    assert_eq!(
        details.create_sql.as_deref(),
        Some("CREATE TABLE demo(id INTEGER PRIMARY KEY)")
    );

    let _ = fs::remove_file(main_path);
    let _ = fs::remove_file(attached_path);
}

fn temp_db_path(label: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("squid-schema-{label}-{stamp}.sqlite"))
}
