use std::time::{SystemTime, UNIX_EPOCH};
use std::path::Path;

use super::{
    RecentStore, normalize_database_path, path_to_sqlite_uri_path, recent_path_is_available,
};

#[test]
fn load_from_path_ignores_blank_lines() {
    let path = unique_test_path("load");
    std::fs::write(&path, "\nC:\\db1.sqlite\n\nC:\\db2.sqlite\n").unwrap();

    let paths = RecentStore::load_from_path(&path).unwrap();

    assert_eq!(paths.len(), 2);
    cleanup(&path);
}

#[test]
fn load_from_path_reports_non_not_found_errors() {
    let path = unique_test_path("load-dir");
    std::fs::create_dir_all(&path).unwrap();

    let error = RecentStore::load_from_path(&path).unwrap_err();

    assert!(
        error
            .to_string()
            .contains("failed to read recent database list")
    );
    let _ = std::fs::remove_dir(&path);
}

#[test]
fn load_from_path_preserves_surrounding_whitespace() {
    let path = unique_test_path("load-whitespace");
    std::fs::write(&path, " report.db\nreport.db \n").unwrap();

    let paths = RecentStore::load_from_path(&path).unwrap();

    assert_eq!(
        paths,
        vec![
            std::path::PathBuf::from(" report.db"),
            std::path::PathBuf::from("report.db "),
        ]
    );
    cleanup(&path);
}

#[test]
fn save_and_remove_preserve_recent_order() {
    let path = unique_test_path("save");
    let entries = vec![
        std::path::PathBuf::from("C:\\db1.sqlite"),
        std::path::PathBuf::from("C:\\db2.sqlite"),
    ];

    RecentStore::save_to_path(&path, &entries).unwrap();
    let loaded = RecentStore::load_from_path(&path).unwrap();
    assert_eq!(loaded, entries);

    let filtered = loaded
        .into_iter()
        .filter(|entry| entry != &std::path::PathBuf::from("C:\\db1.sqlite"))
        .collect::<Vec<_>>();
    RecentStore::save_to_path(&path, &filtered).unwrap();

    let after = RecentStore::load_from_path(&path).unwrap();
    assert_eq!(after, vec![std::path::PathBuf::from("C:\\db2.sqlite")]);
    cleanup(&path);
}

#[test]
fn record_logic_moves_existing_to_front_and_trims() {
    let mut entries = (0..12)
        .map(|index| std::path::PathBuf::from(format!("C:\\db{index}.sqlite")))
        .collect::<Vec<_>>();
    let target = std::path::PathBuf::from("C:\\db5.sqlite");

    entries.retain(|entry| entry != &target);
    entries.insert(0, target.clone());
    entries.truncate(10);

    assert_eq!(entries.first(), Some(&target));
    assert_eq!(entries.len(), 10);
}

#[test]
fn normalize_database_path_preserves_memory_databases() {
    let path = std::path::Path::new(":memory:");

    assert_eq!(normalize_database_path(path).unwrap(), path);
}

#[test]
fn normalize_database_path_preserves_sqlite_uri_filenames() {
    let path = std::path::Path::new("file:/tmp/app.db?mode=ro");

    assert_eq!(normalize_database_path(path).unwrap(), path);
}

#[test]
fn normalize_database_path_absolutizes_relative_sqlite_uri_filenames() {
    let path = std::path::Path::new("file:./fixtures/app.db?mode=ro");
    let expected = std::env::current_dir().unwrap().join("./fixtures/app.db");

    assert_eq!(
        normalize_database_path(path).unwrap(),
        std::path::PathBuf::from(format!(
            "file:{}?mode=ro",
            path_to_sqlite_uri_path(&expected)
        ))
    );
}

#[test]
fn normalize_database_path_absolutizes_regular_relative_paths() {
    let path = std::path::Path::new("sakila.db");
    let normalized = normalize_database_path(path).unwrap();

    assert!(normalized.is_absolute());
    assert!(normalized.ends_with(path));
}

#[test]
fn normalize_database_path_collapses_lexical_aliases() {
    let canonical = normalize_database_path(std::path::Path::new("sakila.db")).unwrap();
    let dotted = normalize_database_path(std::path::Path::new("./sakila.db")).unwrap();
    let parent = normalize_database_path(std::path::Path::new("sub/../sakila.db")).unwrap();

    assert_eq!(canonical, dotted);
    assert_eq!(canonical, parent);
}

#[test]
fn recent_path_is_available_for_sqlite_file_uris() {
    let path = std::env::temp_dir().join(format!(
        "squid-available-{}.db",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(&path, b"sqlite").unwrap();

    let uri = std::path::PathBuf::from(format!(
        "file:{}?mode=ro",
        path_to_sqlite_uri_path(&path)
    ));
    assert!(recent_path_is_available(&uri));

    cleanup(&path);
}

#[test]
fn recent_path_is_available_for_localhost_file_uris() {
    let path = std::env::temp_dir().join(format!(
        "squid-localhost-{}.db",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(&path, b"sqlite").unwrap();

    let uri = std::path::PathBuf::from(format!(
        "file://localhost{}?mode=ro",
        path_to_sqlite_uri_path(&path)
    ));
    assert!(recent_path_is_available(&uri));

    cleanup(&path);
}

#[test]
fn recent_path_is_available_for_percent_encoded_file_uris() {
    let path = std::env::temp_dir().join(format!(
        "squid my db {}.sqlite",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(&path, b"sqlite").unwrap();

    let encoded_path = path_to_sqlite_uri_path(&path).replace(' ', "%20");
    let uri = std::path::PathBuf::from(format!("file:{encoded_path}?mode=ro"));
    assert!(recent_path_is_available(&uri));

    cleanup(&path);
}

#[test]
fn normalize_database_path_preserves_memory_file_uris() {
    let path = std::path::Path::new("file::memory:?cache=shared");

    assert_eq!(normalize_database_path(path).unwrap(), path);
}

#[test]
fn to_items_marks_existing_and_missing_paths() {
    let existing = unique_test_path("to-items-existing");
    std::fs::write(&existing, b"sqlite").unwrap();
    let missing = unique_test_path("to-items-missing");

    let items = RecentStore::to_items(vec![existing.clone(), missing.clone()]);
    assert_eq!(items[0].path, existing);
    assert!(items[0].available);
    assert_eq!(items[1].path, missing);
    assert!(!items[1].available);

    cleanup(&items[0].path);
}

#[test]
fn save_to_path_creates_parent_and_preserves_order() {
    let root = unique_test_dir("save-order");
    let storage = root.join("nested").join("recent.txt");
    let entries = vec![
        std::path::PathBuf::from("C:\\db1.sqlite"),
        std::path::PathBuf::from("C:\\db2.sqlite"),
        std::path::PathBuf::from("C:\\db3.sqlite"),
    ];

    RecentStore::save_to_path(&storage, &entries).unwrap();
    let loaded = RecentStore::load_from_path(&storage).unwrap();

    assert_eq!(loaded, entries);
    cleanup(&root);
}

#[test]
fn manual_record_logic_normalizes_dedupes_and_trims() {
    let mut entries = (0..RecentStore::MAX_ITEMS)
        .map(|index| std::path::PathBuf::from(format!("C:\\db{index}.sqlite")))
        .collect::<Vec<_>>();
    let relative = Path::new("./record-relative-test.sqlite");
    let normalized = normalize_database_path(relative).unwrap();

    entries.retain(|existing| existing != &normalized);
    entries.insert(0, normalized.clone());
    entries.truncate(RecentStore::MAX_ITEMS);

    let alt = normalize_database_path(Path::new("record-relative-test.sqlite")).unwrap();
    entries.retain(|existing| existing != &alt);
    entries.insert(0, alt.clone());
    entries.truncate(RecentStore::MAX_ITEMS);

    assert_eq!(entries.len(), RecentStore::MAX_ITEMS);
    assert_eq!(entries.first(), Some(&normalized));
    assert_eq!(entries.iter().filter(|path| **path == normalized).count(), 1);
}

#[test]
fn manual_remove_logic_preserves_order_and_is_noop_when_absent() {
    let first = std::path::PathBuf::from("C:\\db1.sqlite");
    let second = std::path::PathBuf::from("C:\\db2.sqlite");
    let third = std::path::PathBuf::from("C:\\db3.sqlite");
    let mut entries = vec![first.clone(), second.clone(), third.clone()];

    entries.retain(|existing| existing != &second);
    assert_eq!(entries, vec![first.clone(), third.clone()]);

    let absent = std::path::PathBuf::from("C:\\missing.sqlite");
    entries.retain(|existing| existing != &absent);
    assert_eq!(entries, vec![first, third]);
}

#[test]
fn normalize_database_path_keeps_absolute_file_uris_unchanged() {
    let uri = Path::new("file:/tmp/app.db?mode=ro");
    assert_eq!(normalize_database_path(uri).unwrap(), uri);
}

#[test]
fn recent_path_is_unavailable_for_remote_authority_and_bad_encoding() {
    assert!(!recent_path_is_available(Path::new(
        "file://example.com/shared/app.db?mode=ro"
    )));
    assert!(!recent_path_is_available(Path::new("file:/tmp/bad%ZZname.db")));
}

fn unique_test_path(label: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("squid-{label}-{nanos}.txt"))
}

fn unique_test_dir(label: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("squid-{label}-{nanos}"))
}

fn cleanup(path: &std::path::Path) {
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_dir_all(path);
}
