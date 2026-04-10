use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;

use super::{Database, exact_match_score, fuzzy_score};
use crate::db::TableSummary;

#[test]
fn fuzzy_score_prefers_tighter_match() {
    let compact = fuzzy_score("alphabet", "alp").unwrap();
    let loose = fuzzy_score("a long phrase with letters", "alp").unwrap();
    assert!(compact > loose);
}

#[test]
fn exact_match_prefers_full_match_over_prefix() {
    let full = exact_match_score("actor", "actor").unwrap();
    let prefix = exact_match_score("actor_name", "actor").unwrap();
    assert!(full > prefix);
}

#[test]
fn search_table_keeps_hits_when_rowid_aliases_are_shadowed() {
    let path = temp_db_path("search-shadowed-rowid-current");
    let conn = Connection::open(&path).expect("create db");
    conn.execute(
        "CREATE TABLE demo(rowid INTEGER, _rowid_ INTEGER, oid INTEGER, name TEXT)",
        [],
    )
    .expect("create table");
    conn.execute(
        "INSERT INTO demo(rowid, _rowid_, oid, name) VALUES (10, 20, 30, 'alpha')",
        [],
    )
    .expect("insert first");
    conn.execute(
        "INSERT INTO demo(rowid, _rowid_, oid, name) VALUES (11, 21, 31, 'bravo')",
        [],
    )
    .expect("insert second");
    drop(conn);

    let db = Database::open(&path).expect("open db");
    let results = db
        .search_table("demo", &["name".to_string()], &[], &[], "alp", 10)
        .expect("search table");

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].rowid, None);
    assert_eq!(results[0].row_label, "row 1");
    assert_eq!(results[0].values, vec!["alpha"]);
    assert_eq!(results[0].matched_columns, vec![true]);

    let _ = fs::remove_file(path);
}

#[test]
fn search_tables_keeps_hits_when_rowid_aliases_are_shadowed() {
    let path = temp_db_path("search-shadowed-rowid-all");
    let conn = Connection::open(&path).expect("create db");
    conn.execute(
        "CREATE TABLE demo(rowid INTEGER, _rowid_ INTEGER, oid INTEGER, name TEXT)",
        [],
    )
    .expect("create table");
    conn.execute(
        "INSERT INTO demo(rowid, _rowid_, oid, name) VALUES (10, 20, 30, 'alpha')",
        [],
    )
    .expect("insert first");
    conn.execute(
        "INSERT INTO demo(rowid, _rowid_, oid, name) VALUES (11, 21, 31, 'bravo')",
        [],
    )
    .expect("insert second");
    drop(conn);

    let db = Database::open(&path).expect("open db");
    let results = db
        .search_tables(
            &[TableSummary {
                name: "demo".to_string(),
            }],
            "alpha",
            10,
        )
        .expect("search tables");

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].table_name, "demo");
    assert_eq!(results[0].rowid, None);
    assert_eq!(results[0].row_label, "row 1");
    assert!(results[0].haystack.contains("alpha"));

    let _ = fs::remove_file(path);
}

fn temp_db_path(label: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("squid-search-db-{label}-{stamp}.sqlite"))
}
