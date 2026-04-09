use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;
use rusqlite::types::Value;

use super::{Database, FilterClause, FilterMode, SortClause, build_filter_where, build_order_by};

#[test]
fn build_filter_where_uses_all_clauses() {
    let clauses = vec![
        FilterClause {
            column_name: "name".into(),
            mode: FilterMode::Contains,
            value: "sam".into(),
        },
        FilterClause {
            column_name: "active".into(),
            mode: FilterMode::IsTrue,
            value: String::new(),
        },
    ];

    let (sql, params) = build_filter_where(&clauses);
    assert!(sql.contains("\"name\" LIKE ?"));
    assert!(sql.contains("CAST(\"active\" AS INTEGER) <> 0"));
    assert_eq!(params, vec![Value::Text("%sam%".into())]);
}

#[test]
fn build_order_by_keeps_sort_priority() {
    let clauses = vec![
        SortClause {
            column_name: "last_name".into(),
            descending: false,
        },
        SortClause {
            column_name: "created_at".into(),
            descending: true,
        },
    ];

    assert_eq!(
        build_order_by(&clauses),
        " ORDER BY \"last_name\" ASC, \"created_at\" DESC"
    );
}

#[test]
fn row_record_uses_hidden_rowid_alias_when_rowid_column_exists() {
    let path = temp_db_path("query-hidden-rowid");
    let conn = Connection::open(&path).expect("create db");
    conn.execute("CREATE TABLE demo(rowid INTEGER, name TEXT)", [])
        .expect("create table");
    conn.execute("INSERT INTO demo(rowid, name) VALUES (101, 'alpha')", [])
        .expect("insert first");
    conn.execute("INSERT INTO demo(rowid, name) VALUES (202, 'beta')", [])
        .expect("insert second");
    drop(conn);

    let db = Database::open(&path).expect("open db");
    let record = db
        .row_record_at_offset("demo", &[], &[], 0)
        .expect("load row")
        .expect("record");

    assert_eq!(record.rowid, Some(1));
    assert_eq!(record.row_label, "rowid 1");

    let _ = fs::remove_file(path);
}

#[test]
fn row_record_falls_back_to_rowid_when_underscore_rowid_is_shadowed() {
    let path = temp_db_path("query-shadowed-underscore-rowid");
    let conn = Connection::open(&path).expect("create db");
    conn.execute("CREATE TABLE demo(_rowid_ TEXT, name TEXT)", [])
        .expect("create table");
    conn.execute("INSERT INTO demo(_rowid_, name) VALUES ('x', 'alpha')", [])
        .expect("insert first");
    conn.execute("INSERT INTO demo(_rowid_, name) VALUES ('y', 'beta')", [])
        .expect("insert second");
    drop(conn);

    let db = Database::open(&path).expect("open db");
    let record = db
        .row_record_at_offset("demo", &[], &[], 0)
        .expect("load row")
        .expect("record");

    assert_eq!(record.rowid, Some(1));
    assert_eq!(record.row_label, "rowid 1");

    let _ = fs::remove_file(path);
}

fn temp_db_path(label: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("squid-{label}-{stamp}.sqlite"))
}
