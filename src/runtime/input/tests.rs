use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crossterm::event::{KeyCode, KeyEvent};
use rusqlite::Connection;

use super::action_for_key;
use crate::app::{
    Action, App, AppMode, DetailField, DetailPane, DetailState, FilterModalState, FilterPane,
    SearchScope, SearchState,
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
        horizontal_offset: 0,
        result_limit: 10,
        submitted: false,
        loading: false,
    });

    assert_eq!(
        action_for_key(&app, KeyEvent::from(KeyCode::Char('1'))),
        Action::InputChar('1')
    );
}

#[test]
fn search_accepts_horizontal_navigation_keys() {
    let mut app = test_app("search-horizontal");
    app.search = Some(SearchState {
        scope: SearchScope::AllTables,
        query: String::new(),
        results: Vec::new(),
        selected_result: 0,
        result_offset: 0,
        horizontal_offset: 0,
        result_limit: 10,
        submitted: false,
        loading: false,
    });

    assert_eq!(
        action_for_key(&app, KeyEvent::from(KeyCode::Left)),
        Action::MoveLeft
    );
    assert_eq!(
        action_for_key(&app, KeyEvent::from(KeyCode::Right)),
        Action::MoveRight
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

#[test]
fn detail_modal_shortcuts_switch_between_edit_and_save_actions() {
    let mut app = test_app("detail-input");
    app.detail = Some(DetailState {
        rowid: Some(1),
        row_label: "rowid 1".to_string(),
        pane: DetailPane::Value,
        selected_field: 0,
        value_scroll: 0,
        value_view_width: 40,
        value_view_height: 10,
        is_editing: false,
        message: None,
        fields: vec![DetailField {
            column_name: "name".to_string(),
            data_type: "TEXT".to_string(),
            not_null: false,
            original_value: "alice".to_string(),
            draft_value: "alice".to_string(),
            foreign_target: None,
            is_blob: false,
        }],
    });

    assert_eq!(
        action_for_key(&app, KeyEvent::from(KeyCode::Char('e'))),
        Action::EditDetail
    );
    assert_eq!(
        action_for_key(&app, KeyEvent::from(KeyCode::Char('s'))),
        Action::SaveDetail
    );
    assert_eq!(
        action_for_key(&app, KeyEvent::from(KeyCode::Enter)),
        Action::EditDetail
    );

    app.detail.as_mut().unwrap().is_editing = true;
    assert_eq!(
        action_for_key(&app, KeyEvent::from(KeyCode::Char('s'))),
        Action::InputChar('s')
    );
    assert_eq!(
        action_for_key(&app, KeyEvent::from(KeyCode::Enter)),
        Action::NewLine
    );
}

fn test_app(label: &str) -> App {
    let path = temp_db_path(label);
    let conn = Connection::open(&path).expect("create db");
    conn.execute("CREATE TABLE demo(id INTEGER PRIMARY KEY, name TEXT)", [])
        .expect("create table");
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
    std::env::temp_dir().join(format!("squid-input-{label}-{stamp}.sqlite"))
}
