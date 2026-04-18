mod browse;
mod overlays;
mod sql;

use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

use crate::app::{Action, App};
use crate::ui::LayoutInfo;

#[derive(Default)]
pub struct MouseState {
    last_search_click: Option<(usize, Instant)>,
    last_home_click: Option<(usize, Instant)>,
    last_table_row_click: Option<(usize, Instant)>,
    last_sql_history_click: Option<(usize, Instant)>,
    last_left_down: Option<(u16, u16)>,
}

pub fn handle_mouse_event(
    app: &mut App,
    layout: &LayoutInfo,
    mouse: MouseEvent,
    state: &mut MouseState,
    now: Instant,
) -> Result<bool> {
    let left_click = is_left_click(mouse, state);

    app.sync_search_results_view_width(
        layout
            .search_results
            .map(|area| area.width as usize)
            .unwrap_or(0),
    );

    let column = mouse.column;
    let row = mouse.row;

    if left_click {
        if contains(layout.header_tabs.browse, column, row) {
            app.handle(Action::SwitchToBrowse)?;
            return Ok(false);
        }
        if contains(layout.header_tabs.sql, column, row) {
            app.handle(Action::SwitchToSql)?;
            return Ok(false);
        }
        if app.mode == crate::app::AppMode::Sql && contains(layout.header_tabs.run, column, row) {
            app.handle(Action::ExecuteSql)?;
            return Ok(false);
        }
        if contains(layout.header_tabs.quit, column, row) {
            return app.request_quit();
        }
    }

    if sql::handle_sql_mouse(app, layout, mouse, state, now, left_click)? {
        return Ok(false);
    }

    if overlays::handle_modal_mouse(app, layout, mouse, left_click)? {
        return Ok(false);
    }

    if overlays::handle_filter_modal_mouse(app, layout, mouse, left_click)? {
        return Ok(false);
    }

    if overlays::handle_detail_mouse(app, layout, mouse, left_click)? {
        return Ok(false);
    }

    if overlays::handle_search_mouse(app, layout, mouse, state, now, left_click)? {
        return Ok(false);
    }

    browse::handle_browse_mouse(app, layout, mouse, state, now, left_click)?;

    Ok(false)
}

fn handle_search_double_click(app: &mut App, state: &mut MouseState, now: Instant) -> Result<()> {
    if let Some(selected) = app.search.as_ref().map(|search| search.selected_result) {
        if is_double_click(state.last_search_click, selected, now) {
            app.handle(Action::Confirm)?;
            state.last_search_click = None;
        } else {
            state.last_search_click = Some((selected, now));
        }
    }
    Ok(())
}

fn handle_row_double_click(app: &mut App, state: &mut MouseState, now: Instant) -> Result<()> {
    let (selected, previous_click) = if app.is_home() {
        (app.selected_recent, &mut state.last_home_click)
    } else {
        (app.selected_row, &mut state.last_table_row_click)
    };

    if is_double_click(*previous_click, selected, now) {
        app.handle(Action::Confirm)?;
        *previous_click = None;
    } else {
        *previous_click = Some((selected, now));
    }
    Ok(())
}

fn handle_sql_history_double_click(
    app: &mut App,
    state: &mut MouseState,
    now: Instant,
) -> Result<()> {
    let selected = app.sql.selected_history;
    if is_double_click(state.last_sql_history_click, selected, now) {
        app.handle(Action::Confirm)?;
        state.last_sql_history_click = None;
    } else {
        state.last_sql_history_click = Some((selected, now));
    }
    Ok(())
}

fn is_double_click(previous: Option<(usize, Instant)>, selected: usize, now: Instant) -> bool {
    previous.is_some_and(|(last_index, last_time)| {
        last_index == selected && now.duration_since(last_time) <= Duration::from_millis(500)
    })
}

fn is_left_click(mouse: MouseEvent, state: &mut MouseState) -> bool {
    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            state.last_left_down = Some((mouse.column, mouse.row));
            true
        }
        MouseEventKind::Up(MouseButton::Left) => state.last_left_down.take().is_none(),
        _ => false,
    }
}

fn contains(area: ratatui::layout::Rect, column: u16, row: u16) -> bool {
    column >= area.x && column < area.x + area.width && row >= area.y && row < area.y + area.height
}

fn clear_click_state(state: &mut MouseState) {
    state.last_search_click = None;
    clear_row_click_state(state);
}

fn clear_row_click_state(state: &mut MouseState) {
    state.last_home_click = None;
    state.last_table_row_click = None;
}

#[cfg(test)]
mod tests;
