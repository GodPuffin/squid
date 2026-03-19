use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crossterm::event::{KeyCode, KeyEvent};
use rusqlite::Connection;

use super::action_for_key;
use crate::app::{
    Action, App, AppMode, FilterModalState, FilterPane, SearchScope, SearchState,
};

#[test]
fn root_digit_shortcuts_still_switch_modes() {
    let app = test_app("root-digit");

    assert_eq!(
        action_for_key(&app, KeyEvent::from(KeyCode::Char('1'))),
        Action::SwitchToBrowse
    );
    assert_eq!(
        action_for_key(&app, KeyEvent::from(KeyCode::Char('2'))),
        Action::SwitchToSql
    );
}

#[test]
fn search_accepts_numeric_input() {
    let mut app = test_app("search-digit");
    app.search = Some(SearchState {
        scope: SearchScope::CurrentTable,
        query: String::new(),
        results: Vec::new(),
        selected_result: 0,
        result_offset: 0,
        result_limit: 10,
        submitted: false,
    });

    assert_eq!(
        action_for_key(&app, KeyEvent::from(KeyCode::Char('1'))),
        Action::InputChar('1')
    );
}

#[test]
fn filter_draft_accepts_numeric_input() {
    let mut app = test_app("filter-digit");
    app.filter_modal = Some(FilterModalState {
        pane: FilterPane::Draft,
        column_index: 0,
        mode_index: 0,
        active_index: 0,
        input: String::new(),
    });

    assert_eq!(
        action_for_key(&app, KeyEvent::from(KeyCode::Char('2'))),
        Action::InputChar('2')
    );
}

#[test]
fn sql_editor_accepts_q_and_digits_as_text() {
    let mut app = test_app("sql-editor-input");
    app.mode = AppMode::Sql;

    assert_eq!(
        action_for_key(&app, KeyEvent::from(KeyCode::Char('q'))),
        Action::InputChar('q')
    );
    assert_eq!(
        action_for_key(&app, KeyEvent::from(KeyCode::Char('1'))),
        Action::InputChar('1')
    );
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

fn temp_db_path(label: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("squid-input-{label}-{stamp}.sqlite"))
}
