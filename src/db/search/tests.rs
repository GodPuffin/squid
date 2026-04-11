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
fn fuzzy_score_prefers_compact_later_match_over_earlier_scattered_match() {
    let compact = fuzzy_score("A Canadian drama", "candian").unwrap();
    let scattered = fuzzy_score("A cable and indigo ants nearby", "candian").unwrap();
    assert!(compact > scattered);
}

#[test]
fn exact_match_prefers_full_match_over_prefix() {
    let full = exact_match_score("actor", "actor").unwrap();
    let prefix = exact_match_score("actor_name", "actor").unwrap();
    assert!(full > prefix);
}

#[test]
fn search_table_prefers_exact_value_match_over_earlier_fuzzy_match() {
    let path = temp_db_path("search-current-exact-over-fuzzy");
    let conn = Connection::open(&path).expect("create db");
    conn.execute("CREATE TABLE demo(title TEXT, description TEXT)", [])
        .expect("create table");
    conn.execute(
        "INSERT INTO demo(title, description) VALUES (?1, ?2)",
        ("cxaxnxaxdxixaxn", "fuzzy-only"),
    )
    .expect("insert fuzzy row");
    conn.execute(
        "INSERT INTO demo(title, description) VALUES (?1, ?2)",
        ("plain title", "A Canadian drama"),
    )
    .expect("insert exact row");
    drop(conn);

    let db = Database::open(&path).expect("open db");
    let results = db
        .search_table(
            "demo",
            &["title".to_string(), "description".to_string()],
            &[],
            &[],
            "canadian",
            10,
        )
        .expect("search table");

    assert_eq!(results.len(), 2);
    assert_eq!(results[0].values, vec!["plain title", "A Canadian drama"]);
    assert_eq!(results[1].values, vec!["cxaxnxaxdxixaxn", "fuzzy-only"]);

    let _ = fs::remove_file(path);
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
    for idx in 0..1_250 {
        let value = if idx == 1_175 {
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
    for idx in 0..750 {
        let value = if idx == 620 {
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
fn search_tables_keeps_global_top_hits_from_single_table() {
    let path = temp_db_path("search-all-tables-global-top-hits");
    let conn = Connection::open(&path).expect("create db");
    conn.execute("CREATE TABLE best(name TEXT)", [])
        .expect("create best table");
    conn.execute("CREATE TABLE fallback(name TEXT)", [])
        .expect("create fallback table");
    for _ in 0..120 {
        conn.execute("INSERT INTO best(name) VALUES ('needle')", [])
            .expect("insert best row");
    }
    for _ in 0..80 {
        conn.execute("INSERT INTO fallback(name) VALUES ('needle suffix')", [])
            .expect("insert fallback row");
    }
    drop(conn);

    let db = Database::open(&path).expect("open db");
    let results = db
        .search_tables(
            &[
                TableSummary {
                    name: "best".to_string(),
                },
                TableSummary {
                    name: "fallback".to_string(),
                },
            ],
            "needle",
            100,
        )
        .expect("search tables");

    assert_eq!(results.len(), 100);
    assert!(results.iter().all(|hit| hit.table_name == "best"));
    assert!(results.iter().all(|hit| hit.values == vec!["needle"]));

    let _ = fs::remove_file(path);
}

#[test]
fn search_tables_keeps_only_requested_number_of_best_hits() {
    let path = temp_db_path("search-all-tables-limit-best-hits");
    let conn = Connection::open(&path).expect("create db");
    conn.execute("CREATE TABLE best(name TEXT)", [])
        .expect("create best table");
    conn.execute("CREATE TABLE fallback(name TEXT)", [])
        .expect("create fallback table");
    for value in ["needle", "prefix needle", "needle suffix"] {
        conn.execute("INSERT INTO best(name) VALUES (?1)", [value])
            .expect("insert best row");
    }
    for idx in 0..100 {
        let value = format!("needle filler {idx:03}");
        conn.execute("INSERT INTO fallback(name) VALUES (?1)", [&value])
            .expect("insert fallback row");
    }
    drop(conn);

    let db = Database::open(&path).expect("open db");
    let all_results = db
        .search_tables(
            &[
                TableSummary {
                    name: "best".to_string(),
                },
                TableSummary {
                    name: "fallback".to_string(),
                },
            ],
            "needle",
            200,
        )
        .expect("search tables");
    let results = db
        .search_tables(
            &[
                TableSummary {
                    name: "best".to_string(),
                },
                TableSummary {
                    name: "fallback".to_string(),
                },
            ],
            "needle",
            3,
        )
        .expect("search tables");

    assert_eq!(results.len(), 3);
    assert_eq!(
        results
            .iter()
            .map(|hit| (hit.table_name.clone(), hit.values.clone(), hit.score))
            .collect::<Vec<_>>(),
        all_results
            .iter()
            .take(3)
            .map(|hit| (hit.table_name.clone(), hit.values.clone(), hit.score))
            .collect::<Vec<_>>()
    );

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
fn search_table_treats_like_wildcards_as_literal_query_characters() {
    let path = temp_db_path("search-like-wildcards-escaped");
    let conn = Connection::open(&path).expect("create db");
    conn.execute("CREATE TABLE demo(name TEXT)", [])
        .expect("create table");
    conn.execute("INSERT INTO demo(name) VALUES ('100% real')", [])
        .expect("insert first");
    conn.execute("INSERT INTO demo(name) VALUES ('100 percent real')", [])
        .expect("insert second");
    drop(conn);

    let db = Database::open(&path).expect("open db");
    let percent_results = db
        .search_table("demo", &["name".to_string()], &[], &[], "%", 10)
        .expect("search percent");
    let underscore_results = db
        .search_table("demo", &["name".to_string()], &[], &[], "_", 10)
        .expect("search underscore");

    assert_eq!(percent_results.len(), 1);
    assert_eq!(percent_results[0].values, vec!["100% real"]);
    assert!(underscore_results.is_empty());

    let _ = fs::remove_file(path);
}

#[test]
fn search_table_keeps_only_requested_number_of_best_hits() {
    let path = temp_db_path("search-current-table-limits-best-hits");
    let conn = Connection::open(&path).expect("create db");
    conn.execute("CREATE TABLE demo(name TEXT)", [])
        .expect("create table");
    for value in [
        "needle",
        "prefix needle",
        "needle suffix",
        "n e e d l e",
        "far away needle text",
    ] {
        conn.execute("INSERT INTO demo(name) VALUES (?1)", [value])
            .expect("insert row");
    }
    for idx in 0..100 {
        let value = format!("needle filler {idx:03}");
        conn.execute("INSERT INTO demo(name) VALUES (?1)", [&value])
            .expect("insert filler row");
    }
    drop(conn);

    let db = Database::open(&path).expect("open db");
    let all_results = db
        .search_table("demo", &["name".to_string()], &[], &[], "needle", 200)
        .expect("search table");
    let results = db
        .search_table("demo", &["name".to_string()], &[], &[], "needle", 3)
        .expect("search table");

    assert_eq!(results.len(), 3);
    assert!(results.iter().all(|hit| hit.haystack.contains("needle")));
    assert_eq!(
        results
            .iter()
            .map(|hit| (hit.values.clone(), hit.score))
            .collect::<Vec<_>>(),
        all_results
            .iter()
            .take(3)
            .map(|hit| (hit.values.clone(), hit.score))
            .collect::<Vec<_>>()
    );

    let _ = fs::remove_file(path);
}

fn temp_db_path(label: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("squid-search-db-{label}-{stamp}.sqlite"))
}
