use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crossterm::event::{KeyCode, KeyEvent};
use rusqlite::Connection;

use super::action_for_key;
use crate::app::{
    Action, App, AppMode, DetailPane, DetailState, FilterModalState, FilterPane, ModalPane,
    ModalState, SearchScope, SearchState, SqlPane,
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

#[test]
fn detail_state_takes_precedence_over_search_and_root() {
    let mut app = test_app("detail-precedence");
    app.detail = Some(DetailState {
        row_label: "row 1".to_string(),
        pane: DetailPane::Fields,
        selected_field: 0,
        value_scroll: 0,
        value_view_width: 10,
        value_view_height: 4,
        fields: Vec::new(),
    });
    app.search = Some(empty_search());

    assert_eq!(
        action_for_key(&app, KeyEvent::from(KeyCode::Char('q'))),
        Action::CloseModal
    );
}

#[test]
fn search_state_takes_precedence_over_filter_and_root() {
    let mut app = test_app("search-precedence");
    app.search = Some(empty_search());
    app.filter_modal = Some(FilterModalState {
        pane: FilterPane::Columns,
        column_index: 0,
        mode_index: 0,
        active_index: 0,
        input: String::new(),
    });

    assert_eq!(
        action_for_key(&app, KeyEvent::from(KeyCode::Backspace)),
        Action::Backspace
    );
}

#[test]
fn filter_q_closes_outside_draft_but_types_inside_draft() {
    let mut app = test_app("filter-q");
    app.filter_modal = Some(FilterModalState {
        pane: FilterPane::Columns,
        column_index: 0,
        mode_index: 0,
        active_index: 0,
        input: String::new(),
    });
    assert_eq!(
        action_for_key(&app, KeyEvent::from(KeyCode::Char('q'))),
        Action::CloseModal
    );

    app.filter_modal.as_mut().unwrap().pane = FilterPane::Draft;
    assert_eq!(
        action_for_key(&app, KeyEvent::from(KeyCode::Char('q'))),
        Action::InputChar('q')
    );
}

#[test]
fn modal_q_closes_while_root_q_quits() {
    let mut app = test_app("modal-q");
    assert_eq!(
        action_for_key(&app, KeyEvent::from(KeyCode::Char('q'))),
        Action::Quit
    );

    app.modal = Some(ModalState {
        pane: ModalPane::Columns,
        column_index: 0,
        sort_column_index: 0,
        sort_active_index: 0,
        pending_desc: false,
    });
    assert_eq!(
        action_for_key(&app, KeyEvent::from(KeyCode::Char('q'))),
        Action::CloseModal
    );
}

#[test]
fn sql_mode_routes_commands_by_focus() {
    let mut app = test_app("sql-routing");
    app.mode = AppMode::Sql;
    app.sql.focus = SqlPane::Editor;

    assert_eq!(
        action_for_key(&app, KeyEvent::from(KeyCode::Char('q'))),
        Action::InputChar('q')
    );
    assert_eq!(
        action_for_key(&app, KeyEvent::from(KeyCode::Char('1'))),
        Action::InputChar('1')
    );
    assert_eq!(
        action_for_key(&app, KeyEvent::from(KeyCode::Char('c'))),
        Action::InputChar('c')
    );

    app.sql.focus = SqlPane::History;
    assert_eq!(
        action_for_key(&app, KeyEvent::from(KeyCode::Char('q'))),
        Action::Quit
    );
    assert_eq!(
        action_for_key(&app, KeyEvent::from(KeyCode::Char('1'))),
        Action::SwitchToBrowse
    );
    assert_eq!(
        action_for_key(&app, KeyEvent::from(KeyCode::Char('c'))),
        Action::Clear
    );
}

#[test]
fn escape_closes_active_layer_before_root_behavior() {
    let mut app = test_app("escape-close");
    app.search = Some(empty_search());
    assert_eq!(
        action_for_key(&app, KeyEvent::from(KeyCode::Esc)),
        Action::CloseModal
    );
}

fn empty_search() -> SearchState {
    SearchState {
        scope: SearchScope::CurrentTable,
        query: String::new(),
        results: Vec::new(),
        selected_result: 0,
        result_offset: 0,
        result_limit: 10,
        submitted: false,
    }
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
