use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use rusqlite::Connection;

use crate::app::{Action, App, AppMode, SqlCompletionItem, SqlCompletionState, SqlPane, SqlState};
use crate::ui::layout_info;

use super::{MouseState, contains, handle_mouse_event, is_double_click};

#[test]
fn is_double_click_requires_same_index_within_threshold() {
    let now = Instant::now();
    assert!(is_double_click(Some((1, now)), 1, now + Duration::from_millis(400)));
    assert!(!is_double_click(Some((1, now)), 2, now + Duration::from_millis(400)));
    assert!(!is_double_click(Some((1, now)), 1, now + Duration::from_millis(700)));
}

#[test]
fn contains_checks_rect_bounds() {
    let area = Rect::new(10, 5, 4, 3);
    assert!(contains(area, 10, 5));
    assert!(contains(area, 13, 7));
    assert!(!contains(area, 14, 7));
    assert!(!contains(area, 13, 8));
}

#[test]
fn header_clicks_switch_modes() {
    let mut app = app_with_mouse_data("mouse-header");
    app.mode = AppMode::Sql;
    app.sql.focus = SqlPane::History;
    let layout = layout_info(Rect::new(0, 0, 80, 24), &app);
    let mut state = MouseState::default();

    handle_mouse_event(
        &mut app,
        &layout,
        mouse_down(layout.header_tabs.browse.x, layout.header_tabs.browse.y),
        &mut state,
        Instant::now(),
    )
    .unwrap();
    assert_eq!(app.mode, AppMode::Browse);

    handle_mouse_event(
        &mut app,
        &layout,
        mouse_down(layout.header_tabs.sql.x, layout.header_tabs.sql.y),
        &mut state,
        Instant::now(),
    )
    .unwrap();
    assert_eq!(app.mode, AppMode::Sql);
}

#[test]
fn sql_completion_click_applies_selected_item() {
    let mut app = app_with_mouse_data("mouse-sql");
    app.mode = AppMode::Sql;
    app.sql = SqlState {
        query: "SEL".to_string(),
        cursor: 3,
        editor_scroll: 0,
        editor_col_offset: 0,
        editor_height: 5,
        editor_width: 20,
        focus: SqlPane::Editor,
        history: Vec::new(),
        history_offset: 0,
        history_height: 5,
        selected_history: 0,
        result: crate::app::SqlResultState::Empty,
        result_scroll: 0,
        result_height: 5,
        completion: Some(SqlCompletionState {
            prefix_start: 0,
            items: vec![SqlCompletionItem {
                label: "SELECT".to_string(),
                insert_text: "SELECT".to_string(),
            }],
            selected: 0,
        }),
        status: String::new(),
        column_cache: std::collections::HashMap::new(),
    };

    let layout = layout_info(Rect::new(0, 0, 80, 24), &app);
    let completion = layout.sql.as_ref().and_then(|sql| sql.completion).unwrap();
    let mut state = MouseState::default();

    handle_mouse_event(
        &mut app,
        &layout,
        mouse_down(completion.x + 1, completion.y + 1),
        &mut state,
        Instant::now(),
    )
    .unwrap();

    assert_eq!(app.sql.query, "SELECT");
    assert!(app.sql.completion.is_none());
}

#[test]
fn modal_and_filter_clicks_update_selection() {
    let mut app = app_with_mouse_data("mouse-modal");
    app.handle(Action::OpenConfig).unwrap();
    let mut state = MouseState::default();

    let modal_layout = layout_info(Rect::new(0, 0, 80, 24), &app);
    let sort_candidates = modal_layout.modal.as_ref().unwrap().sort_candidates;
    handle_mouse_event(
        &mut app,
        &modal_layout,
        mouse_down(sort_candidates.x + 1, sort_candidates.y + 1),
        &mut state,
        Instant::now(),
    )
    .unwrap();
    assert_ne!(app.modal_sort_active_lines(), vec!["No active sort"]);

    app.handle(Action::OpenFilters).unwrap();
    let filter_layout = layout_info(Rect::new(0, 0, 80, 24), &app);
    let modes = filter_layout.filter_modal.as_ref().unwrap().modes;
    handle_mouse_event(
        &mut app,
        &filter_layout,
        mouse_down(modes.x + 1, modes.y + 1),
        &mut state,
        Instant::now(),
    )
    .unwrap();
    assert_eq!(app.filter_modal_pane(), Some(crate::app::FilterPane::Modes));
}

fn app_with_mouse_data(label: &str) -> App {
    let path = temp_db_path(label);
    let conn = Connection::open(&path).expect("create db");
    conn.execute_batch(
        "CREATE TABLE demo(name TEXT, age INTEGER);
         INSERT INTO demo(name, age) VALUES ('alice', 30), ('bob', 40);",
    )
    .expect("seed db");
    drop(conn);

    let app = App::load(path.clone()).expect("load app");
    let _ = fs::remove_file(path);
    app
}

fn mouse_down(column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column,
        row,
        modifiers: KeyModifiers::NONE,
    }
}

fn temp_db_path(label: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("squid-mouse-{label}-{stamp}.sqlite"))
}
