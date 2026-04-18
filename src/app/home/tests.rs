#[cfg(unix)]
use std::os::unix::ffi::OsStringExt;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::app::{AppMode, ContentView, PaneFocus, SqlHistoryEntry, SqlPane};
use crate::db::FilterMode;

use super::{
    AppStorage, StoredFilterRule, StoredSession, StoredSortRule, StoredTableState,
    normalize_database_path, path_to_sqlite_uri_path, recent_path_is_available, recent_paths_match,
};

#[test]
fn recent_storage_round_trips_and_keeps_order() {
    let storage = unique_test_path("recent-storage", "db");
    let first = unique_test_path("recent-first", "sqlite");
    let second = unique_test_path("recent-second", "sqlite");

    AppStorage::record_recent_at(&storage, &first).unwrap();
    AppStorage::record_recent_at(&storage, &second).unwrap();
    AppStorage::record_recent_at(&storage, &first).unwrap();

    let loaded = AppStorage::load_recent_from_path(&storage, 10).unwrap();
    let loaded_paths = loaded.into_iter().map(|item| item.path).collect::<Vec<_>>();

    assert_eq!(loaded_paths, vec![first.clone(), second.clone()]);

    AppStorage::remove_recent_at(&storage, &first).unwrap();
    let after = AppStorage::load_recent_from_path(&storage, 10).unwrap();
    let after_paths = after.into_iter().map(|item| item.path).collect::<Vec<_>>();
    assert_eq!(after_paths, vec![second]);

    cleanup(&storage);
}

#[test]
fn session_storage_round_trips_query_history_and_table_state() {
    let storage = unique_test_path("session-storage", "db");
    let database = unique_test_path("session-demo", "sqlite");
    let session = StoredSession {
        mode: AppMode::Sql,
        focus: PaneFocus::Content,
        content_view: ContentView::Schema,
        selected_table_name: Some("main.users".to_string()),
        selected_row: 7,
        selected_row_rowid: Some(41),
        row_offset: 5,
        schema_offset: 3,
        sql_query: "select * from users".to_string(),
        sql_cursor: 6,
        sql_focus: SqlPane::Results,
        sql_history: vec![SqlHistoryEntry {
            query: "select 1".to_string(),
            summary: "Returned 1 row(s)".to_string(),
        }],
        table_states: vec![StoredTableState {
            table_name: "main.users".to_string(),
            hidden_columns: vec!["email".to_string()],
            sort_rules: vec![StoredSortRule {
                column_name: "name".to_string(),
                descending: true,
            }],
            filter_rules: vec![StoredFilterRule {
                column_name: "active".to_string(),
                mode: FilterMode::IsTrue,
                value: String::new(),
            }],
        }],
    };

    AppStorage::save_session_at(&storage, &database, &session).unwrap();

    let loaded = AppStorage::load_session_at(&storage, &database)
        .unwrap()
        .expect("stored session");

    assert_eq!(loaded.mode, AppMode::Sql);
    assert_eq!(loaded.focus, PaneFocus::Content);
    assert_eq!(loaded.content_view, ContentView::Schema);
    assert_eq!(loaded.selected_table_name.as_deref(), Some("main.users"));
    assert_eq!(loaded.selected_row, 7);
    assert_eq!(loaded.selected_row_rowid, Some(41));
    assert_eq!(loaded.row_offset, 5);
    assert_eq!(loaded.schema_offset, 3);
    assert_eq!(loaded.sql_query, "select * from users");
    assert_eq!(loaded.sql_cursor, 6);
    assert_eq!(loaded.sql_focus, SqlPane::Results);
    assert_eq!(loaded.sql_history.len(), 1);
    assert_eq!(loaded.table_states.len(), 1);
    assert_eq!(
        loaded.table_states[0].hidden_columns,
        vec!["email".to_string()]
    );
    assert_eq!(loaded.table_states[0].sort_rules[0].column_name, "name");
    assert!(loaded.table_states[0].sort_rules[0].descending);
    assert_eq!(
        loaded.table_states[0].filter_rules[0].mode,
        FilterMode::IsTrue
    );

    let last_opened = AppStorage::last_opened_path_at(&storage).unwrap();
    assert_eq!(
        last_opened,
        Some(normalize_database_path(&database).unwrap())
    );

    cleanup(&storage);
}

#[test]
fn recent_paths_match_plain_paths_and_file_uri_aliases() {
    let path = std::env::temp_dir().join("squid-recent-match.db");
    let raw_path = path.clone();
    let file_uri =
        std::path::PathBuf::from(format!("file:{}?mode=ro", path_to_sqlite_uri_path(&path)));
    let localhost_uri = std::path::PathBuf::from(format!(
        "file://localhost{}?mode=ro",
        path_to_sqlite_uri_path(&path)
    ));

    assert!(recent_paths_match(&raw_path, &file_uri));
    assert!(recent_paths_match(&file_uri, &localhost_uri));
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

    let uri = std::path::PathBuf::from(format!("file:{}?mode=ro", path_to_sqlite_uri_path(&path)));
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

#[cfg(unix)]
#[test]
fn path_storage_round_trips_non_utf8_paths() {
    let path = std::path::PathBuf::from(std::ffi::OsString::from_vec(vec![
        b'r', b'e', b'p', b'o', b'r', b't', 0xff, b'.', b'd', b'b',
    ]));

    let bytes = super::path_to_storage_bytes(&path);
    let decoded = super::path_from_storage_bytes(&bytes).unwrap();

    assert_eq!(decoded, path);
}

fn unique_test_path(label: &str, extension: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("squid-{label}-{nanos}.{extension}"))
}

fn cleanup(path: &std::path::Path) {
    let _ = std::fs::remove_file(path);
}
