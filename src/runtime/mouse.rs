use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

use crate::app::{Action, App};
use crate::ui::{self, LayoutInfo};

#[derive(Default)]
pub struct MouseState {
    last_search_click: Option<(usize, Instant)>,
    last_row_click: Option<(usize, Instant)>,
}

pub fn handle_mouse_event(
    app: &mut App,
    layout: &LayoutInfo,
    mouse: MouseEvent,
    state: &mut MouseState,
    now: Instant,
) -> Result<()> {
    let column = mouse.column;
    let row = mouse.row;

    if let Some(modal) = &layout.modal {
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(index) = ui::list_row_at(modal.columns, column, row) {
                    app.modal_click_columns(index)?;
                } else if let Some(index) = ui::list_row_at(modal.sort_candidates, column, row) {
                    app.modal_click_sort_candidate(index, false)?;
                } else if let Some(index) = ui::list_row_at(modal.sort_stack, column, row) {
                    app.modal_select_sort_rule(index);
                }
            }
            MouseEventKind::Down(MouseButton::Right) => {
                if let Some(index) = ui::list_row_at(modal.sort_candidates, column, row) {
                    app.modal_click_sort_candidate(index, true)?;
                } else if let Some(index) = ui::list_row_at(modal.sort_stack, column, row) {
                    app.modal_remove_sort_rule(index)?;
                }
            }
            MouseEventKind::ScrollUp => scroll_modal_hit(
                app,
                [
                    ui::list_row_at(modal.columns, column, row),
                    ui::list_row_at(modal.sort_candidates, column, row),
                    ui::list_row_at(modal.sort_stack, column, row),
                ],
                Action::MoveUp,
            )?,
            MouseEventKind::ScrollDown => scroll_modal_hit(
                app,
                [
                    ui::list_row_at(modal.columns, column, row),
                    ui::list_row_at(modal.sort_candidates, column, row),
                    ui::list_row_at(modal.sort_stack, column, row),
                ],
                Action::MoveDown,
            )?,
            _ => {}
        }
        return Ok(());
    }

    if let Some(filter_modal) = &layout.filter_modal {
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(index) = ui::list_row_at(filter_modal.columns, column, row) {
                    app.filter_modal_select_column(index);
                } else if let Some(index) = ui::list_row_at(filter_modal.modes, column, row) {
                    app.filter_modal_select_mode(index);
                } else if contains(filter_modal.draft, column, row) {
                    app.filter_modal_focus_draft();
                } else if let Some(index) = ui::list_row_at(filter_modal.active, column, row) {
                    app.filter_modal_select_active(index);
                }
            }
            MouseEventKind::ScrollUp => {
                if let Some(index) = ui::list_row_at(filter_modal.columns, column, row) {
                    app.filter_modal_select_column(index);
                    app.handle(Action::MoveUp)?;
                } else if let Some(index) = ui::list_row_at(filter_modal.modes, column, row) {
                    app.filter_modal_select_mode(index);
                    app.handle(Action::MoveUp)?;
                } else if contains(filter_modal.draft, column, row) {
                    app.filter_modal_focus_draft();
                } else if let Some(index) = ui::list_row_at(filter_modal.active, column, row) {
                    app.filter_modal_select_active(index);
                    app.handle(Action::MoveUp)?;
                }
            }
            MouseEventKind::ScrollDown => {
                if let Some(index) = ui::list_row_at(filter_modal.columns, column, row) {
                    app.filter_modal_select_column(index);
                    app.handle(Action::MoveDown)?;
                } else if let Some(index) = ui::list_row_at(filter_modal.modes, column, row) {
                    app.filter_modal_select_mode(index);
                    app.handle(Action::MoveDown)?;
                } else if contains(filter_modal.draft, column, row) {
                    app.filter_modal_focus_draft();
                } else if let Some(index) = ui::list_row_at(filter_modal.active, column, row) {
                    app.filter_modal_select_active(index);
                    app.handle(Action::MoveDown)?;
                }
            }
            _ => {}
        }
        return Ok(());
    }

    if let Some(detail) = &layout.detail {
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(index) = ui::list_row_at(detail.fields, column, row) {
                    app.detail_select_field(index);
                } else if contains(detail.value, column, row) {
                    app.detail_focus_value();
                }
            }
            MouseEventKind::ScrollUp => {
                if let Some(index) = ui::list_row_at(detail.fields, column, row) {
                    app.detail_select_field(index);
                    app.handle(Action::MoveUp)?;
                } else if contains(detail.value, column, row) {
                    app.detail_focus_value();
                    app.detail_scroll_value(-1);
                }
            }
            MouseEventKind::ScrollDown => {
                if let Some(index) = ui::list_row_at(detail.fields, column, row) {
                    app.detail_select_field(index);
                    app.handle(Action::MoveDown)?;
                } else if contains(detail.value, column, row) {
                    app.detail_focus_value();
                    app.detail_scroll_value(1);
                }
            }
            _ => {}
        }
        return Ok(());
    }

    if let (Some(search_box), Some(search_results)) = (&layout.search_box, &layout.search_results) {
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(index) = ui::search_result_row_at(*search_results, column, row) {
                    app.select_search_result_in_view(index);
                    handle_search_double_click(app, state, now)?;
                    return Ok(());
                }
                if contains(*search_box, column, row) {
                    app.focus_content();
                    state.last_search_click = None;
                    return Ok(());
                }
                state.last_search_click = None;
            }
            MouseEventKind::ScrollUp if contains(*search_results, column, row) => {
                app.scroll_search(-1);
                state.last_search_click = None;
                return Ok(());
            }
            MouseEventKind::ScrollDown if contains(*search_results, column, row) => {
                app.scroll_search(1);
                state.last_search_click = None;
                return Ok(());
            }
            _ => {}
        }
    }

    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            if let Some(index) = ui::list_row_at(layout.tables, column, row) {
                app.select_table_by_index(index)?;
                if app.is_home() {
                    handle_row_double_click(app, state, now)?;
                } else {
                    state.last_row_click = None;
                }
            } else if let Some(index) = ui::table_row_at(layout.content, column, row) {
                app.focus_content();
                app.select_row_in_view(index)?;
                handle_row_double_click(app, state, now)?;
            } else if contains(layout.content, column, row) {
                app.focus_content();
                state.last_row_click = None;
            } else {
                state.last_row_click = None;
            }
        }
        MouseEventKind::ScrollUp => {
            state.last_row_click = None;
            if ui::list_row_at(layout.tables, column, row).is_some() {
                app.scroll_tables(-1)?;
            } else if contains(layout.content, column, row) {
                app.scroll_content(-1)?;
            }
        }
        MouseEventKind::ScrollDown => {
            state.last_row_click = None;
            if ui::list_row_at(layout.tables, column, row).is_some() {
                app.scroll_tables(1)?;
            } else if contains(layout.content, column, row) {
                app.scroll_content(1)?;
            }
        }
        _ => {}
    }

    Ok(())
}

fn scroll_modal_hit(app: &mut App, hits: [Option<usize>; 3], action: Action) -> Result<()> {
    if hits.iter().any(Option::is_some) {
        app.handle(action)?;
    }
    Ok(())
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
    let selected = if app.is_home() {
        app.selected_recent
    } else {
        app.selected_row
    };
    if is_double_click(state.last_row_click, selected, now) {
        app.handle(Action::Confirm)?;
        state.last_row_click = None;
    } else {
        state.last_row_click = Some((selected, now));
    }
    Ok(())
}

fn is_double_click(previous: Option<(usize, Instant)>, selected: usize, now: Instant) -> bool {
    previous.is_some_and(|(last_index, last_time)| {
        last_index == selected && now.duration_since(last_time) <= Duration::from_millis(500)
    })
}

fn contains(area: ratatui::layout::Rect, column: u16, row: u16) -> bool {
    column >= area.x && column < area.x + area.width && row >= area.y && row < area.y + area.height
}
