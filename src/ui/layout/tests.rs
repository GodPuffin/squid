use crate::app::{App, AppMode, SqlCompletionItem, SqlCompletionState};

use super::{Rect, home_recent_row_at, layout_info, list_scroll_offset, sql_completion_rect};

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
fn refresh_view_dependent_rects_tracks_sql_viewport_changes() {
    let mut app = App::load(None).expect("load app");
    app.mode = AppMode::Sql;
    app.sql.query = (0..16)
        .map(|index| format!("line{index}"))
        .collect::<Vec<_>>()
        .join("\n");
    app.sql.cursor = app
        .sql
        .query
        .lines()
        .take(10)
        .map(|line| line.len() + 1)
        .sum::<usize>()
        + 3;
    app.sql.editor_scroll = 6;
    app.sql.completion = Some(SqlCompletionState {
        prefix_start: app.sql.cursor,
        items: vec![SqlCompletionItem {
            label: "line_item".to_string(),
            insert_text: "line_item".to_string(),
        }],
        selected: 0,
    });

    let mut layout = layout_info(Rect::new(0, 0, 80, 50), &app);
    let stale_completion = layout.sql.as_ref().unwrap().completion.unwrap();
    let sql = layout.sql.as_ref().unwrap();
    app.set_sql_viewport_sizes(
        sql.editor.height.saturating_sub(2) as usize,
        sql.editor.width.saturating_sub(2) as usize,
        sql.history.height.saturating_sub(2) as usize,
        sql.results.height.saturating_sub(3) as usize,
    );

    layout.refresh_view_dependent_rects(&app);

    let sql = layout.sql.as_ref().unwrap();
    let refreshed_completion = sql.completion.unwrap();
    let (line, _) = app.sql_cursor_line_col();
    let expected_completion = sql_completion_rect(
        sql.editor,
        line.saturating_sub(app.sql.editor_scroll),
        app.sql_cursor_screen_col(),
    );
    assert_ne!(stale_completion, refreshed_completion);
    assert_eq!(refreshed_completion, expected_completion);
}
