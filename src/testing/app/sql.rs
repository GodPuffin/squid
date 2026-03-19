use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;

use super::{
    Action, SqlCompletionItem, SqlCompletionState, SqlHistoryEntry, SqlPane, SqlResultState,
    completion_prefix, completion_qualifier, line_col_from_index, move_vertical,
};
use crate::app::App;

#[test]
fn completion_prefix_reads_identifier_prefix() {
    let query = "SELECT ac";
    let (start, prefix) = completion_prefix(query, query.len());
    assert_eq!(start, 7);
    assert_eq!(prefix, "ac");
}

#[test]
fn line_column_round_trips() {
    let query = "SELECT\nname";
    assert_eq!(line_col_from_index(query, 0), (0, 0));
    assert_eq!(line_col_from_index(query, 7), (1, 0));
}

#[test]
fn vertical_movement_preserves_column_when_possible() {
    let query = "SELECT\ncolumn\nx";
    let moved = move_vertical(query, query.len() - 1, -1);
    assert_eq!(line_col_from_index(query, moved), (1, 0));
}

#[test]
fn completion_qualifier_keeps_table_or_alias_prefix() {
    assert_eq!(completion_qualifier("orders."), "orders.");
    assert_eq!(completion_qualifier("o.id"), "o.");
    assert_eq!(completion_qualifier("id"), "");
}

#[test]
fn sql_completion_preserves_qualified_prefix_when_applied() {
    let path = temp_db_path("qualified-completion");
    let conn = Connection::open(&path).expect("create db");
    conn.execute("CREATE TABLE orders(id INTEGER PRIMARY KEY, name TEXT)", [])
        .expect("create table");
    drop(conn);

    let mut app = App::load(Some(path.clone())).expect("load app");
    app.sql.query = "SELECT orders.".to_string();
    app.sql.cursor = app.sql.query.len();
    app.sql_refresh_completion().expect("refresh completion");
    let completion = app.sql.completion.as_mut().expect("completion");
    completion.selected = completion
        .items
        .iter()
        .position(|item| item.label == "orders.id")
        .expect("orders.id completion");

    app.sql_apply_completion();

    assert_eq!(app.sql.query, "SELECT orders.id");

    let _ = fs::remove_file(path);
}

#[test]
fn sql_completion_matches_alias_qualified_prefixes() {
    let path = temp_db_path("alias-completion");
    let conn = Connection::open(&path).expect("create db");
    conn.execute("CREATE TABLE orders(id INTEGER PRIMARY KEY, name TEXT)", [])
        .expect("create table");
    drop(conn);

    let mut app = App::load(Some(path.clone())).expect("load app");
    app.sql.query = "SELECT o.".to_string();
    app.sql.cursor = app.sql.query.len();
    app.sql_refresh_completion().expect("refresh completion");

    let items = app
        .sql
        .completion
        .as_ref()
        .expect("completion")
        .items
        .iter()
        .map(|item| item.insert_text.as_str())
        .collect::<Vec<_>>();

    assert!(items.contains(&"o.id"));
    assert!(items.contains(&"o.name"));

    let _ = fs::remove_file(path);
}

#[test]
fn sql_completion_matches_unqualified_column_prefixes() {
    let path = temp_db_path("column-prefix-completion");
    let conn = Connection::open(&path).expect("create db");
    conn.execute("CREATE TABLE orders(id INTEGER PRIMARY KEY, name TEXT)", [])
        .expect("create table");
    drop(conn);

    let mut app = App::load(Some(path.clone())).expect("load app");
    app.sql.query = "SELECT na".to_string();
    app.sql.cursor = app.sql.query.len();
    app.sql_refresh_completion().expect("refresh completion");

    let items = app
        .sql
        .completion
        .as_ref()
        .expect("completion")
        .items
        .iter()
        .map(|item| item.insert_text.as_str())
        .collect::<Vec<_>>();

    assert!(items.contains(&"name"));

    let _ = fs::remove_file(path);
}

#[test]
fn sql_execute_reloads_after_insert_returning() {
    let path = temp_db_path("insert-returning");
    let conn = Connection::open(&path).expect("create db");
    conn.execute("CREATE TABLE demo(id INTEGER PRIMARY KEY, name TEXT)", [])
        .expect("create table");
    drop(conn);

    let mut app = App::load(Some(path.clone())).expect("load app");
    assert_eq!(app.preview.total_rows, 0);

    app.sql.query = "INSERT INTO demo(name) VALUES ('delta') RETURNING id".to_string();
    app.sql.cursor = app.sql.query.len();
    app.sql_execute().expect("execute sql");

    assert_eq!(app.preview.total_rows, 1);

    let _ = fs::remove_file(path);
}

#[test]
fn sql_execute_preserves_connection_scoped_state() {
    let path = temp_db_path("connection-state");
    let conn = Connection::open(&path).expect("create db");
    conn.execute("CREATE TABLE demo(id INTEGER PRIMARY KEY, name TEXT)", [])
        .expect("create table");
    drop(conn);

    let mut app = App::load(Some(path.clone())).expect("load app");

    app.sql.query = "CREATE TEMP TABLE temp_demo(value TEXT)".to_string();
    app.sql.cursor = app.sql.query.len();
    app.sql_execute().expect("create temp table");

    app.sql.query = "INSERT INTO temp_demo(value) VALUES ('kept')".to_string();
    app.sql.cursor = app.sql.query.len();
    app.sql_execute().expect("insert temp row");

    app.sql.query = "SELECT value FROM temp_demo".to_string();
    app.sql.cursor = app.sql.query.len();
    app.sql_execute().expect("select temp row");

    match &app.sql.result {
        crate::app::SqlResultState::Rows { columns, rows } => {
            assert_eq!(columns, &vec!["value".to_string()]);
            assert_eq!(rows, &vec![vec!["kept".to_string()]]);
        }
        result => panic!("expected rows, got {result:?}"),
    }

    let _ = fs::remove_file(path);
}

#[test]
fn sql_execute_keeps_temp_tables_visible_in_browse() {
    let path = temp_db_path("temp-browse");
    let conn = Connection::open(&path).expect("create db");
    conn.execute("CREATE TABLE demo(id INTEGER PRIMARY KEY, name TEXT)", [])
        .expect("create table");
    drop(conn);

    let mut app = App::load(Some(path.clone())).expect("load app");

    app.sql.query = "CREATE TEMP TABLE temp_demo(value TEXT)".to_string();
    app.sql.cursor = app.sql.query.len();
    app.sql_execute().expect("create temp table");

    let temp_index = app
        .tables
        .iter()
        .position(|table| table.name == "temp_demo")
        .expect("temp table should be listed");
    app.selected_table = temp_index;
    app.refresh_preview().expect("refresh temp preview");

    assert_eq!(app.selected_table_name(), Some("temp_demo"));
    assert_eq!(app.preview.total_rows, 0);
    assert_eq!(
        app.details
            .as_ref()
            .and_then(|details| details.create_sql.as_deref()),
        Some("CREATE TABLE temp_demo(value TEXT)")
    );

    let _ = fs::remove_file(path);
}

#[test]
fn sql_focus_cycle_moves_forward_and_back() {
    let mut app = test_app("focus-cycle");
    app.mode = crate::app::AppMode::Sql;

    app.handle_sql(Action::ToggleFocus).unwrap();
    assert_eq!(app.sql.focus, SqlPane::History);
    app.handle_sql(Action::ToggleFocus).unwrap();
    assert_eq!(app.sql.focus, SqlPane::Results);
    app.handle_sql(Action::ToggleFocus).unwrap();
    assert_eq!(app.sql.focus, SqlPane::Editor);

    app.handle_sql(Action::ReverseFocus).unwrap();
    assert_eq!(app.sql.focus, SqlPane::Results);
    app.handle_sql(Action::ReverseFocus).unwrap();
    assert_eq!(app.sql.focus, SqlPane::History);
}

#[test]
fn focus_changes_clear_completion_popup() {
    let mut app = test_app("focus-clears-completion");
    app.mode = crate::app::AppMode::Sql;
    app.sql.completion = Some(sample_completion());

    app.handle_sql(Action::ToggleFocus).unwrap();

    assert_eq!(app.sql.focus, SqlPane::History);
    assert!(app.sql.completion.is_none());
}

#[test]
fn newline_applies_completion_or_inserts_line_break() {
    let mut app = test_app("newline");
    app.mode = crate::app::AppMode::Sql;
    app.sql.query = "SEL".to_string();
    app.sql.cursor = app.sql.query.len();
    app.sql.completion = Some(SqlCompletionState {
        prefix_start: 0,
        items: vec![SqlCompletionItem {
            label: "SELECT".to_string(),
            insert_text: "SELECT".to_string(),
        }],
        selected: 0,
    });

    app.handle_sql(Action::NewLine).unwrap();
    assert_eq!(app.sql.query, "SELECT");
    assert_eq!(app.sql.cursor, app.sql.query.len());

    app.handle_sql(Action::NewLine).unwrap();
    assert_eq!(app.sql.query, "SELECT\n");
}

#[test]
fn confirm_loads_selected_history_entry() {
    let mut app = test_app("confirm-history");
    app.mode = crate::app::AppMode::Sql;
    app.sql.focus = SqlPane::History;
    app.sql.history = vec![
        SqlHistoryEntry {
            query: "SELECT 1".to_string(),
            summary: "Rows: 1".to_string(),
        },
        SqlHistoryEntry {
            query: "SELECT 2".to_string(),
            summary: "Rows: 1".to_string(),
        },
    ];
    app.sql.selected_history = 1;

    app.handle_sql(Action::Confirm).unwrap();

    assert_eq!(app.sql.query, "SELECT 2");
    assert_eq!(app.sql.focus, SqlPane::Editor);
    assert_eq!(app.sql.cursor, app.sql.query.len());
}

#[test]
fn clear_only_resets_active_pane_state() {
    let mut app = test_app("clear-pane");
    app.mode = crate::app::AppMode::Sql;
    app.sql.query = "SELECT 1".to_string();
    app.sql.cursor = app.sql.query.len();
    app.sql.history = vec![SqlHistoryEntry {
        query: "SELECT 2".to_string(),
        summary: "Rows: 1".to_string(),
    }];
    app.sql.result = SqlResultState::Message {
        text: "ok".to_string(),
        is_error: false,
    };

    app.sql.focus = SqlPane::Editor;
    app.handle_sql(Action::Clear).unwrap();
    assert!(app.sql.query.is_empty());
    assert_eq!(app.sql.history.len(), 1);

    app.sql.focus = SqlPane::History;
    app.handle_sql(Action::Clear).unwrap();
    assert!(app.sql.history.is_empty());
    assert!(matches!(app.sql.result, SqlResultState::Message { .. }));

    app.sql.focus = SqlPane::Results;
    app.handle_sql(Action::Clear).unwrap();
    assert!(matches!(app.sql.result, SqlResultState::Empty));
}

#[test]
fn ensure_sql_viewport_clamps_editor_history_and_results() {
    let mut app = test_app("viewport-clamp");
    app.sql.query = "one\ntwo\nthree\nfour\nfive".to_string();
    app.sql.cursor = app.sql.query.len();
    app.sql.editor_height = 2;
    app.sql.editor_scroll = 99;
    app.sql.history = (0..5)
        .map(|idx| SqlHistoryEntry {
            query: format!("SELECT {idx}"),
            summary: "Rows: 1".to_string(),
        })
        .collect();
    app.sql.history_height = 2;
    app.sql.selected_history = 4;
    app.sql.history_offset = 99;
    app.sql.result = SqlResultState::Rows {
        columns: vec!["name".to_string()],
        rows: (0..5).map(|idx| vec![idx.to_string()]).collect(),
    };
    app.sql.result_height = 2;
    app.sql.result_scroll = 99;

    app.ensure_sql_viewport();

    assert_eq!(app.sql.editor_scroll, 3);
    assert_eq!(app.sql.history_offset, 3);
    assert_eq!(app.sql.result_scroll, 3);
}

#[test]
fn selecting_history_or_completion_ignores_out_of_range_indices() {
    let mut app = test_app("select-range");
    app.sql.history = vec![SqlHistoryEntry {
        query: "SELECT 1".to_string(),
        summary: "Rows: 1".to_string(),
    }];
    app.sql.selected_history = 0;
    app.sql.completion = Some(sample_completion());

    app.sql_select_history_in_view(10);
    assert_eq!(app.sql.selected_history, 0);

    app.sql_select_completion_in_view(10);
    assert_eq!(app.sql.completion.as_ref().unwrap().selected, 0);
}

#[test]
fn refresh_completion_requires_editor_focus_and_nonempty_prefix() {
    let mut app = test_app("refresh-completion");
    app.sql.query = "".to_string();
    app.sql.cursor = 0;
    app.sql.completion = Some(sample_completion());
    app.sql_refresh_completion().unwrap();
    assert!(app.sql.completion.is_none());

    app.sql.focus = SqlPane::History;
    app.sql.completion = Some(sample_completion());
    app.sql.query = "SEL".to_string();
    app.sql.cursor = app.sql.query.len();
    app.sql_refresh_completion().unwrap();
    assert!(app.sql.completion.is_some());
}

#[test]
fn public_focus_helpers_update_focus() {
    let mut app = test_app("focus-helpers");
    app.sql_focus_results();
    assert_eq!(app.sql.focus, SqlPane::Results);
    app.sql_focus_editor();
    assert_eq!(app.sql.focus, SqlPane::Editor);
}

fn sample_completion() -> SqlCompletionState {
    SqlCompletionState {
        prefix_start: 0,
        items: vec![
            SqlCompletionItem {
                label: "SELECT".to_string(),
                insert_text: "SELECT".to_string(),
            },
            SqlCompletionItem {
                label: "SET".to_string(),
                insert_text: "SET".to_string(),
            },
        ],
        selected: 0,
    }
}

fn temp_db_path(label: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("squid-sql-{label}-{stamp}.sqlite"))
}

fn test_app(label: &str) -> App {
    let path = temp_db_path(label);
    let conn = Connection::open(&path).expect("create db");
    conn.execute("CREATE TABLE demo(id INTEGER PRIMARY KEY, name TEXT)", [])
        .expect("create table");
    drop(conn);

    let app = App::load(Some(path.clone())).expect("load app");
    let _ = fs::remove_file(path);
    app
}
