use std::time::Instant;

use anyhow::Result;
use crossterm::event::{MouseEvent, MouseEventKind};

use crate::app::{Action, App};
use crate::ui::{self, LayoutInfo};

use super::{MouseState, contains, handle_sql_history_double_click};

pub(super) fn handle_sql_mouse(
    app: &mut App,
    layout: &LayoutInfo,
    mouse: MouseEvent,
    state: &mut MouseState,
    now: Instant,
    left_click: bool,
) -> Result<bool> {
    let Some(sql) = &layout.sql else {
        return Ok(false);
    };
    let column = mouse.column;
    let row = mouse.row;

    if left_click {
        if let Some(completion) = sql.completion
            && let Some(index) = ui::list_row_at(completion, column, row)
        {
            let visible_items = completion.height.saturating_sub(2) as usize;
            app.sql_select_completion_in_view(index, visible_items);
            app.sql_apply_selected_completion();
            state.last_sql_history_click = None;
        } else if contains(sql.editor, column, row) {
            let line_in_view = row.saturating_sub(sql.editor.y + 1) as usize;
            let col_in_view = column.saturating_sub(sql.editor.x) as usize;
            app.sql_set_cursor_from_view(line_in_view, col_in_view);
            state.last_sql_history_click = None;
        } else if let Some(index) = ui::list_row_at(sql.history, column, row) {
            app.sql_focus_history();
            app.sql_select_history_in_view(index);
            handle_sql_history_double_click(app, state, now)?;
        } else if contains(sql.history, column, row) {
            app.sql_focus_history();
            state.last_sql_history_click = None;
        } else if contains(sql.results, column, row) {
            app.sql_focus_results();
            state.last_sql_history_click = None;
        }
    } else {
        match mouse.kind {
            MouseEventKind::ScrollUp => {
                if let Some(completion) = sql.completion
                    && contains(completion, column, row)
                {
                    app.handle(Action::MoveUp)?;
                } else if let Some(index) = ui::list_row_at(sql.history, column, row) {
                    app.sql_select_history_in_view(index);
                    app.handle(Action::MoveUp)?;
                } else if contains(sql.history, column, row) {
                    app.sql_focus_history();
                    app.handle(Action::MoveUp)?;
                } else if contains(sql.editor, column, row) {
                    app.sql_focus_editor();
                    app.handle(Action::MoveUp)?;
                } else if contains(sql.results, column, row) {
                    app.sql_focus_results();
                    app.handle(Action::MoveUp)?;
                }
                state.last_sql_history_click = None;
            }
            MouseEventKind::ScrollDown => {
                if let Some(completion) = sql.completion
                    && contains(completion, column, row)
                {
                    app.handle(Action::MoveDown)?;
                } else if let Some(index) = ui::list_row_at(sql.history, column, row) {
                    app.sql_select_history_in_view(index);
                    app.handle(Action::MoveDown)?;
                } else if contains(sql.history, column, row) {
                    app.sql_focus_history();
                    app.handle(Action::MoveDown)?;
                } else if contains(sql.editor, column, row) {
                    app.sql_focus_editor();
                    app.handle(Action::MoveDown)?;
                } else if contains(sql.results, column, row) {
                    app.sql_focus_results();
                    app.handle(Action::MoveDown)?;
                }
                state.last_sql_history_click = None;
            }
            _ => {}
        }
    }

    Ok(true)
}
