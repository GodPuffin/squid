use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;

use super::{
    Rect, header_tab_rects, home_recent_row_at, layout_info, list_scroll_offset,
    sql_completion_rect,
};
use crate::app::{App, AppMode, SqlCompletionItem, SqlCompletionState};

#[test]
fn list_scroll_offset_keeps_selected_row_visible() {
    let area = Rect::new(0, 0, 20, 9);

    assert_eq!(list_scroll_offset(area, 0, 12), 0);
    assert_eq!(list_scroll_offset(area, 6, 12), 0);
    assert_eq!(list_scroll_offset(area, 7, 12), 1);
    assert_eq!(list_scroll_offset(area, 11, 12), 5);
}

#[test]
fn home_recent_row_at_applies_scroll_offset() {
    let area = Rect::new(0, 0, 20, 9);

    assert_eq!(home_recent_row_at(area, 1, 1, 8, 12), Some(2));
    assert_eq!(home_recent_row_at(area, 1, 7, 8, 12), Some(8));
}

#[test]
fn layout_info_home_mode_has_no_sql_or_search_panels() {
    let app = App::load(None).expect("load app");
    let layout = layout_info(Rect::new(0, 0, 120, 40), &app);

    assert!(layout.sql.is_none());
    assert!(layout.search_box.is_none());
    assert!(layout.search_results.is_none());
}

#[test]
fn layout_info_browse_mode_splits_tables_and_content() {
    let app = test_app("browse-layout");
    let layout = layout_info(Rect::new(0, 0, 120, 40), &app);

    assert!(layout.sql.is_none());
    assert!(layout.tables.width > 0);
    assert!(layout.content.width > 0);
}

#[test]
fn layout_info_sql_mode_populates_sql_rects() {
    let mut app = test_app("sql-layout");
    app.mode = AppMode::Sql;

    let layout = layout_info(Rect::new(0, 0, 120, 40), &app);
    let sql = layout.sql.expect("sql rects");

    assert_eq!(layout.tables, Rect::default());
    assert!(sql.editor.width > 0);
    assert!(sql.history.width > 0);
    assert!(sql.results.width > 0);
}

#[test]
fn layout_info_sql_mode_clamps_completion_popup_inside_editor() {
    let mut app = test_app("sql-completion-layout");
    app.mode = AppMode::Sql;
    app.sql.query = "SELECT very_long_table_name".to_string();
    app.sql.cursor = app.sql.query.len();
    app.sql.editor_scroll = 20;
    app.sql.completion = Some(SqlCompletionState {
        prefix_start: 7,
        items: vec![SqlCompletionItem {
            label: "very_long_table_name".to_string(),
            insert_text: "very_long_table_name".to_string(),
        }],
        selected: 0,
    });

    let layout = layout_info(Rect::new(0, 0, 60, 18), &app);
    let sql = layout.sql.expect("sql");
    let popup = sql.completion.expect("completion popup");

    assert!(popup.x >= sql.editor.x);
    assert!(popup.y >= sql.editor.y);
    assert!(popup.x + popup.width <= sql.editor.x + sql.editor.width);
    assert!(popup.y + popup.height <= sql.editor.y + sql.editor.height);
}

#[test]
fn header_tabs_are_ordered_and_do_not_overlap() {
    let tabs = header_tab_rects(Rect::new(0, 0, 120, 3));
    assert!(tabs.browse.x < tabs.sql.x);
    assert!(tabs.browse.x + tabs.browse.width <= tabs.sql.x);
}

#[test]
fn sql_completion_rect_clamps_near_right_and_bottom_edges() {
    let editor = Rect::new(10, 5, 30, 10);
    let popup = sql_completion_rect(editor, 20, 100);

    assert!(popup.x + popup.width <= editor.x + editor.width);
    assert!(popup.y + popup.height <= editor.y + editor.height);
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
    std::env::temp_dir().join(format!("squid-layout-{label}-{stamp}.sqlite"))
}
