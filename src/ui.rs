mod chrome;
mod content;
mod layout;
mod modals;
mod search;
mod sql;
mod tables;
mod widgets;

use ratatui::Frame;

use crate::app::App;

pub use layout::{
    LayoutInfo, home_recent_row_at, layout_info, list_row_at, list_scroll_offset,
    search_result_row_at, table_row_at, viewport_sizes,
};

pub fn render(frame: &mut Frame, app: &App) {
    let layout = layout_info(frame.area(), app);

    if app.is_home() {
        content::render(frame, app, &layout);
        return;
    }

    chrome::render_header(frame, app, &layout);
    if app.mode == crate::app::AppMode::Browse {
        tables::render_tables(frame, app, layout.tables);
        content::render(frame, app, &layout);
    } else {
        sql::render(frame, app, &layout);
    }
    chrome::render_footer(frame, app, layout.footer);
    modals::render(frame, app, &layout);
}
