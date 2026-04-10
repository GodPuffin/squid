use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;

use super::{Database, bounded_scan_limit, exact_match_score, fuzzy_score};
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

#[test]
fn search_table_uses_integer_primary_key_named_rowid_even_when_other_aliases_are_shadowed() {
    let path = temp_db_path("search-explicit-rowid-primary-key");
    let conn = Connection::open(&path).expect("create db");
    conn.execute(
        "CREATE TABLE demo(rowid INTEGER PRIMARY KEY, _rowid_ TEXT, oid TEXT, name TEXT)",
        [],
    )
    .expect("create table");
    conn.execute(
        "INSERT INTO demo(_rowid_, oid, name) VALUES ('shadow', 'shadow-oid', 'alpha')",
        [],
    )
    .expect("insert row");
    drop(conn);

    let db = Database::open(&path).expect("open db");
    let results = db
        .search_table("demo", &["name".to_string()], &[], &[], "alp", 10)
        .expect("search table");

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].rowid, Some(1));
    assert_eq!(results[0].row_label, "rowid 1");

    let _ = fs::remove_file(path);
}

#[test]
fn search_table_finds_match_beyond_previous_scan_window() {
    let path = temp_db_path("search-current-table-exhaustive");
    let conn = Connection::open(&path).expect("create db");
    conn.execute("CREATE TABLE demo(name TEXT)", [])
        .expect("create table");
    for idx in 0..200 {
        let value = if idx == 175 {
            "match target"
        } else {
            "filler value"
        };
        conn.execute("INSERT INTO demo(name) VALUES (?1)", [value])
            .expect("insert row");
    }
    drop(conn);

    let db = Database::open(&path).expect("open db");
    let results = db
        .search_table("demo", &["name".to_string()], &[], &[], "target", 10)
        .expect("search table");

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].values, vec!["match target"]);

    let _ = fs::remove_file(path);
}

#[test]
fn search_tables_finds_match_beyond_previous_scan_window() {
    let path = temp_db_path("search-all-tables-exhaustive");
    let conn = Connection::open(&path).expect("create db");
    conn.execute("CREATE TABLE demo(name TEXT)", [])
        .expect("create table");
    for idx in 0..250 {
        let value = if idx == 220 {
            "cross table match"
        } else {
            "filler value"
        };
        conn.execute("INSERT INTO demo(name) VALUES (?1)", [value])
            .expect("insert row");
    }
    drop(conn);

    let db = Database::open(&path).expect("open db");
    let results = db
        .search_tables(
            &[TableSummary {
                name: "demo".to_string(),
            }],
            "cross table match",
            10,
        )
        .expect("search tables");

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].values, vec!["cross table match"]);

    let _ = fs::remove_file(path);
}

#[test]
fn search_table_scores_against_full_values_not_truncated_preview() {
    let path = temp_db_path("search-full-value-scoring");
    let conn = Connection::open(&path).expect("create db");
    conn.execute("CREATE TABLE demo(note TEXT)", [])
        .expect("create table");
    let long_text = format!("{}needle", "a".repeat(120));
    conn.execute("INSERT INTO demo(note) VALUES (?1)", [&long_text])
        .expect("insert row");
    drop(conn);

    let db = Database::open(&path).expect("open db");
    let results = db
        .search_table("demo", &["note".to_string()], &[], &[], "needle", 10)
        .expect("search table");

    assert_eq!(results.len(), 1);
    assert!(results[0].haystack.contains("needle"));

    let _ = fs::remove_file(path);
}

#[test]
fn bounded_scan_limit_preserves_overscan_for_large_requested_limits() {
    let scan_limit = bounded_scan_limit(30_000, 100, 1_000, 25_000);
    assert_eq!(scan_limit, 3_000_000);
}

#[test]
fn bounded_scan_limit_does_not_hard_cap_when_limit_requires_more_than_cap() {
    let scan_limit = bounded_scan_limit(3_000, 100, 1_000, 25_000);
    assert_eq!(scan_limit, 300_000);
}

#[test]
fn bounded_scan_limit_respects_requested_limit_when_multiplier_is_zero() {
    let scan_limit = bounded_scan_limit(3_000, 0, 1_000, 25_000);
    assert_eq!(scan_limit, 3_000);
}

fn temp_db_path(label: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("squid-search-db-{label}-{stamp}.sqlite"))
}
