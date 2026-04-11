use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;

use crate::app::{Action, App, SearchScope};
use crate::db::SearchHit;

#[test]
fn opening_current_table_search_populates_live_results() {
    let mut app = app_with_search_data("search-current");

    app.open_search(SearchScope::CurrentTable).unwrap();
    app.handle_search(Action::InputChar('a')).unwrap();

    let search = app.search.as_ref().unwrap();
    assert!(search.submitted);
    assert!(!search.results.is_empty());
}

#[test]
fn all_tables_search_starts_unsubmitted_and_confirm_runs_it() {
    let mut app = app_with_search_data("search-all");

    app.open_search(SearchScope::AllTables).unwrap();
    assert!(!app.search.as_ref().unwrap().submitted);

    app.handle_search(Action::InputChar('a')).unwrap();
    assert!(app.search.as_ref().unwrap().results.is_empty());

    app.handle_search(Action::Confirm).unwrap();
    assert!(app.search.as_ref().unwrap().loading);
    assert!(!app.search.as_ref().unwrap().submitted);

    assert!(app.run_pending_work().unwrap());
    assert!(app.search.as_ref().unwrap().submitted);
    assert!(!app.search.as_ref().unwrap().loading);
    assert!(!app.search.as_ref().unwrap().results.is_empty());
}

#[test]
fn large_current_table_search_starts_unsubmitted_and_confirm_runs_it() {
    let path = temp_db_path("search-current-large");
    let conn = Connection::open(&path).expect("create db");
    conn.execute("CREATE TABLE demo(name TEXT)", [])
        .expect("create table");
    for idx in 0..2_100 {
        let value = if idx == 2_050 {
            "target match"
        } else {
            "filler value"
        };
        conn.execute("INSERT INTO demo(name) VALUES (?1)", [value])
            .expect("insert row");
    }
    drop(conn);

    let mut app = App::load(path.clone()).expect("load app");
    app.open_search(SearchScope::CurrentTable).unwrap();
    assert!(!app.search.as_ref().unwrap().submitted);

    app.handle_search(Action::InputChar('t')).unwrap();
    assert!(!app.search.as_ref().unwrap().submitted);
    assert!(app.search.as_ref().unwrap().results.is_empty());

    app.handle_search(Action::Confirm).unwrap();
    assert!(app.search.as_ref().unwrap().loading);
    assert!(!app.search.as_ref().unwrap().submitted);

    assert!(app.run_pending_work().unwrap());
    assert!(app.search.as_ref().unwrap().submitted);
    assert!(!app.search.as_ref().unwrap().loading);
    assert!(!app.search.as_ref().unwrap().results.is_empty());

    let _ = fs::remove_file(path);
}

#[test]
fn editing_all_tables_query_clears_stale_results() {
    let mut app = app_with_search_data("search-stale");

    app.open_search(SearchScope::AllTables).unwrap();
    app.search.as_mut().unwrap().query = "alice".to_string();
    app.handle_search(Action::Confirm).unwrap();
    assert!(app.run_pending_work().unwrap());
    assert!(app.search.as_ref().unwrap().submitted);

    app.handle_search(Action::InputChar('x')).unwrap();
    let search = app.search.as_ref().unwrap();
    assert!(!search.submitted);
    assert!(search.results.is_empty());
    assert_eq!(search.selected_result, 0);
}

#[test]
fn select_result_in_view_uses_absolute_index_and_confirm_jumps() {
    let mut app = app_with_search_data("search-jump");
    app.open_search(SearchScope::AllTables).unwrap();

    {
        let search = app.search.as_mut().unwrap();
        search.results = vec![
            SearchHit {
                table_name: "main.customers".to_string(),
                rowid: Some(1),
                row_offset: 0,
                row_label: "rowid 1".to_string(),
                values: Vec::new(),
                matched_columns: Vec::new(),
                haystack: "alice".to_string(),
                score: 10,
            },
            SearchHit {
                table_name: "main.orders".to_string(),
                rowid: Some(1),
                row_offset: 0,
                row_label: "rowid 1".to_string(),
                values: Vec::new(),
                matched_columns: Vec::new(),
                haystack: "alpha order".to_string(),
                score: 9,
            },
        ];
        search.submitted = true;
        search.result_limit = 1;
        search.result_offset = 1;
    }
    app.select_search_result_in_view(0);
    assert_eq!(app.search.as_ref().unwrap().selected_result, 1);

    app.handle_search(Action::Confirm).unwrap();
    assert!(app.search.is_none());
    assert_eq!(app.focus, crate::app::PaneFocus::Content);
}

#[test]
fn current_table_search_can_jump_without_rowid_alias() {
    let path = temp_db_path("search-shadowed-jump");
    let conn = Connection::open(&path).expect("create db");
    conn.execute_batch(
        "CREATE TABLE demo(rowid INTEGER, _rowid_ INTEGER, oid INTEGER, name TEXT);
         INSERT INTO demo(rowid, _rowid_, oid, name) VALUES
             (10, 20, 30, 'alpha'),
             (11, 21, 31, 'bravo');",
    )
    .expect("seed db");
    drop(conn);

    let mut app = App::load(path.clone()).expect("load app");
    app.focus_content();
    app.open_search(SearchScope::CurrentTable).unwrap();
    app.handle_search(Action::InputChar('b')).unwrap();
    app.handle_search(Action::InputChar('r')).unwrap();
    app.handle_search(Action::InputChar('v')).unwrap();
    app.handle_search(Action::Confirm).unwrap();

    assert!(app.search.is_none());
    assert_eq!(app.selected_row, 1);

    let _ = fs::remove_file(path);
}

#[test]
fn all_tables_search_can_jump_without_rowid_alias() {
    let path = temp_db_path("search-shadowed-all-jump");
    let conn = Connection::open(&path).expect("create db");
    conn.execute_batch(
        "CREATE TABLE demo(rowid INTEGER, _rowid_ INTEGER, oid INTEGER, name TEXT);
         INSERT INTO demo(rowid, _rowid_, oid, name) VALUES
             (10, 20, 30, 'alpha'),
             (11, 21, 31, 'bravo');",
    )
    .expect("seed db");
    drop(conn);

    let mut app = App::load(path.clone()).expect("load app");
    app.focus_content();
    app.open_search(SearchScope::AllTables).unwrap();
    app.handle_search(Action::InputChar('b')).unwrap();
    app.handle_search(Action::InputChar('r')).unwrap();
    app.handle_search(Action::InputChar('a')).unwrap();
    app.handle_search(Action::InputChar('v')).unwrap();
    app.handle_search(Action::InputChar('o')).unwrap();
    app.handle_search(Action::Confirm).unwrap();
    assert!(app.run_pending_work().unwrap());
    app.handle_search(Action::Confirm).unwrap();

    assert!(app.search.is_none());
    assert_eq!(app.selected_row, 1);

    let _ = fs::remove_file(path);
}

#[test]
fn current_table_search_move_clamps_at_bounds() {
    let mut app = app_with_search_data("search-clamp");
    app.open_search(SearchScope::CurrentTable).unwrap();
    app.search.as_mut().unwrap().query = "a".to_string();
    app.handle_search(Action::Confirm).unwrap();

    app.handle_search(Action::MoveUp).unwrap();
    assert_eq!(app.search.as_ref().unwrap().selected_result, 0);

    for _ in 0..10 {
        app.handle_search(Action::MoveDown).unwrap();
    }
    let search = app.search.as_ref().unwrap();
    assert_eq!(
        search.selected_result,
        search.results.len().saturating_sub(1)
    );
}

fn app_with_search_data(label: &str) -> App {
    let path = temp_db_path(label);
    let conn = Connection::open(&path).expect("create db");
    conn.execute_batch(
        "CREATE TABLE customers(id INTEGER PRIMARY KEY, name TEXT);
         CREATE TABLE orders(id INTEGER PRIMARY KEY, customer_id INTEGER, note TEXT);
         INSERT INTO customers(name) VALUES ('alice'), ('bravo'), ('carol');
         INSERT INTO orders(customer_id, note) VALUES (1, 'alpha order'), (2, 'beta order');",
    )
    .expect("seed db");
    drop(conn);

    let app = App::load(path.clone()).expect("load app");
    let _ = fs::remove_file(path);
    app
}

fn temp_db_path(label: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("squid-search-{label}-{stamp}.sqlite"))
}
