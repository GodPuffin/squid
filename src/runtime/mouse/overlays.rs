use std::time::Instant;

use anyhow::Result;
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

use crate::app::{Action, App, DetailPane, SearchScope};
use crate::ui::{self, LayoutInfo, detail_action_rects};

use super::{MouseState, clear_click_state, contains, handle_search_double_click};

pub(super) fn handle_modal_mouse(
    app: &mut App,
    layout: &LayoutInfo,
    mouse: MouseEvent,
    left_click: bool,
) -> Result<bool> {
    let Some(modal) = &layout.modal else {
        return Ok(false);
    };
    let column = mouse.column;
    let row = mouse.row;

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
                } else if let Some(index) = ui::list_row_at(modal.sort_candidates, column, row) {
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

    Ok(true)
}

pub(super) fn handle_filter_modal_mouse(
    app: &mut App,
    layout: &LayoutInfo,
    mouse: MouseEvent,
    left_click: bool,
) -> Result<bool> {
    let Some(filter_modal) = &layout.filter_modal else {
        return Ok(false);
    };
    let column = mouse.column;
    let row = mouse.row;

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

    Ok(true)
}

pub(super) fn handle_detail_mouse(
    app: &mut App,
    layout: &LayoutInfo,
    mouse: MouseEvent,
    left_click: bool,
) -> Result<bool> {
    let Some(detail) = &layout.detail else {
        return Ok(false);
    };
    let column = mouse.column;
    let row = mouse.row;

    if left_click {
        if !contains(detail.area, column, row) {
            app.handle(Action::CloseModal)?;
        } else {
            let buttons = detail_action_rects(detail.header, detail.footer);
            if app.detail_has_changes() && contains(buttons.header_save, column, row) {
                app.handle(Action::SaveDetail)?;
            } else if app.detail_has_changes() && contains(buttons.header_discard, column, row) {
                app.handle(Action::DiscardDetail)?;
            } else if let Some(index) = ui::list_row_at(detail.fields, column, row) {
                app.detail_select_field(index);
            } else if contains(detail.value, column, row) {
                if app.detail_is_editing() {
                    return Ok(true);
                }
                let should_edit = app.detail_pane() == Some(DetailPane::Value)
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

    Ok(true)
}

pub(super) fn handle_search_mouse(
    app: &mut App,
    layout: &LayoutInfo,
    mouse: MouseEvent,
    state: &mut MouseState,
    now: Instant,
    left_click: bool,
) -> Result<bool> {
    let (Some(search_box), Some(search_results)) = (&layout.search_box, &layout.search_results)
    else {
        return Ok(false);
    };
    let column = mouse.column;
    let row = mouse.row;

    if left_click {
        let search_index = app.search.as_ref().and_then(|search| match search.scope {
            SearchScope::CurrentTable => ui::table_row_at(*search_results, column, row),
            SearchScope::AllTables => ui::list_row_at(*search_results, column, row),
        });
        if let Some(index) = search_index {
            app.select_search_result_in_view(index);
            handle_search_double_click(app, state, now)?;
            return Ok(true);
        }
        if contains(*search_box, column, row) {
            app.focus_content();
            clear_click_state(state);
            return Ok(true);
        }
        clear_click_state(state);
        return Ok(false);
    }

    match mouse.kind {
        MouseEventKind::ScrollUp if contains(*search_results, column, row) => {
            app.scroll_search(-1);
            clear_click_state(state);
            Ok(true)
        }
        MouseEventKind::ScrollDown if contains(*search_results, column, row) => {
            app.scroll_search(1);
            clear_click_state(state);
            Ok(true)
        }
        MouseEventKind::ScrollLeft if contains(*search_results, column, row) => {
            app.handle(Action::MoveLeft)?;
            clear_click_state(state);
            Ok(true)
        }
        MouseEventKind::ScrollRight if contains(*search_results, column, row) => {
            app.handle(Action::MoveRight)?;
            clear_click_state(state);
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn scroll_modal_hit(app: &mut App, hits: [Option<usize>; 3], action: Action) -> Result<()> {
    if hits.iter().any(Option::is_some) {
        app.handle(action)?;
    }
    Ok(())
}
