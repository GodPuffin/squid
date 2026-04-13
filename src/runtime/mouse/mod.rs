use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

use crate::app::{Action, App};
use crate::ui::{self, LayoutInfo, detail_action_rects};

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

    if let Some(sql) = &layout.sql {
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
        return Ok(false);
    }

    if let Some(modal) = &layout.modal {
        if left_click {
            if !contains(modal.area, column, row) {
                app.handle(Action::CloseModal)?;
            } else if let Some(index) = ui::list_row_at(modal.columns, column, row) {
                app.modal_click_columns(index)?;
            } else if let Some(index) = ui::list_row_at(modal.sort_candidates, column, row) {
                app.modal_click_sort_candidate(index, false)?;
            } else if let Some(index) = ui::list_row_at(modal.sort_stack, column, row) {
                app.modal_select_sort_rule(index);
            }
        } else {
            match mouse.kind {
                MouseEventKind::Down(MouseButton::Right) => {
                    if !contains(modal.area, column, row) {
                        app.handle(Action::CloseModal)?;
                    } else if let Some(index) = ui::list_row_at(modal.sort_candidates, column, row)
                    {
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
        }
        return Ok(false);
    }

    if let Some(filter_modal) = &layout.filter_modal {
        if left_click {
            if !contains(filter_modal.area, column, row) {
                app.handle(Action::CloseModal)?;
            } else if let Some(index) = ui::list_row_at(filter_modal.columns, column, row) {
                app.filter_modal_select_column(index);
            } else if let Some(index) = ui::list_row_at(filter_modal.modes, column, row) {
                app.filter_modal_select_mode(index);
            } else if contains(filter_modal.draft, column, row) {
                app.filter_modal_focus_draft();
            } else if let Some(index) = ui::list_row_at(filter_modal.active, column, row) {
                app.filter_modal_select_active(index);
            }
        } else {
            match mouse.kind {
                MouseEventKind::Down(MouseButton::Right) => {
                    if !contains(filter_modal.area, column, row) {
                        app.handle(Action::CloseModal)?;
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
        }
        return Ok(false);
    }

    if let Some(detail) = &layout.detail {
        if left_click {
            if !contains(detail.area, column, row) {
                app.handle(Action::CloseModal)?;
            } else {
                let buttons = detail_action_rects(detail.header, detail.footer);
                if app.detail_has_changes() && contains(buttons.header_save, column, row) {
                    app.handle(Action::SaveDetail)?;
                } else if app.detail_has_changes() && contains(buttons.header_discard, column, row)
                {
                    app.handle(Action::DiscardDetail)?;
                } else if let Some(index) = ui::list_row_at(detail.fields, column, row) {
                    app.detail_select_field(index);
                } else if contains(detail.value, column, row) {
                    if app.detail_is_editing() {
                        return Ok(false);
                    }
                    let should_edit = app.detail_pane() == Some(crate::app::DetailPane::Value)
                        && !app.detail_is_editing()
                        && app.detail_selected_field_is_editable();
                    app.detail_focus_value();
                    if should_edit {
                        app.handle(Action::EditDetail)?;
                    }
                }
            }
        } else {
            match mouse.kind {
                MouseEventKind::Down(MouseButton::Right) => {
                    if !contains(detail.area, column, row) {
                        app.handle(Action::CloseModal)?;
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
        }
        return Ok(false);
    }

    if let (Some(search_box), Some(search_results)) = (&layout.search_box, &layout.search_results) {
        if left_click {
            let search_index = app.search.as_ref().and_then(|search| match search.scope {
                crate::app::SearchScope::CurrentTable => {
                    ui::table_row_at(*search_results, column, row)
                }
                crate::app::SearchScope::AllTables => ui::list_row_at(*search_results, column, row),
            });
            if let Some(index) = search_index {
                app.select_search_result_in_view(index);
                handle_search_double_click(app, state, now)?;
                return Ok(false);
            }
            if contains(*search_box, column, row) {
                app.focus_content();
                clear_click_state(state);
                return Ok(false);
            }
            clear_click_state(state);
        } else {
            match mouse.kind {
                MouseEventKind::ScrollUp if contains(*search_results, column, row) => {
                    app.scroll_search(-1);
                    clear_click_state(state);
                    return Ok(false);
                }
                MouseEventKind::ScrollDown if contains(*search_results, column, row) => {
                    app.scroll_search(1);
                    clear_click_state(state);
                    return Ok(false);
                }
                MouseEventKind::ScrollLeft if contains(*search_results, column, row) => {
                    app.handle(Action::MoveLeft)?;
                    clear_click_state(state);
                    return Ok(false);
                }
                MouseEventKind::ScrollRight if contains(*search_results, column, row) => {
                    app.handle(Action::MoveRight)?;
                    clear_click_state(state);
                    return Ok(false);
                }
                _ => {}
            }
        }
    }

    if left_click {
        let table_index = if app.is_home() {
            ui::home_recent_row_at(
                layout.tables,
                column,
                row,
                app.selected_recent,
                app.recent_items.len(),
            )
        } else {
            ui::list_row_at(layout.tables, column, row)
        };

        if let Some(index) = table_index {
            app.select_table_by_index(index)?;
            if app.is_home() {
                handle_row_double_click(app, state, now)?;
            } else {
                clear_row_click_state(state);
            }
        } else if let Some(index) = ui::table_row_at(layout.content, column, row) {
            app.focus_content();
            app.select_row_in_view(index)?;
            handle_row_double_click(app, state, now)?;
        } else if contains(layout.content, column, row) {
            app.focus_content();
            clear_row_click_state(state);
        } else {
            clear_row_click_state(state);
        }
    } else {
        match mouse.kind {
            MouseEventKind::ScrollUp => {
                clear_row_click_state(state);
                if ui::list_row_at(layout.tables, column, row).is_some() {
                    app.scroll_tables(-1)?;
                } else if contains(layout.content, column, row) {
                    app.scroll_content(-1)?;
                }
            }
            MouseEventKind::ScrollDown => {
                clear_row_click_state(state);
                if ui::list_row_at(layout.tables, column, row).is_some() {
                    app.scroll_tables(1)?;
                } else if contains(layout.content, column, row) {
                    app.scroll_content(1)?;
                }
            }
            _ => {}
        }
    }

    Ok(false)
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
