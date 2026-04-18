use std::time::Instant;

use anyhow::Result;
use crossterm::event::{MouseEvent, MouseEventKind};

use crate::app::App;
use crate::ui::{self, LayoutInfo};

use super::{MouseState, clear_row_click_state, contains, handle_row_double_click};

pub(super) fn handle_browse_mouse(
    app: &mut App,
    layout: &LayoutInfo,
    mouse: MouseEvent,
    state: &mut MouseState,
    now: Instant,
    left_click: bool,
) -> Result<()> {
    let column = mouse.column;
    let row = mouse.row;

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

    Ok(())
}
