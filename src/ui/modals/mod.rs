pub mod shared;

pub(crate) mod detail;
mod filter;
mod view;

use ratatui::Frame;

use crate::app::App;

use super::LayoutInfo;

pub fn render(frame: &mut Frame, app: &App, layout: &LayoutInfo) {
    if app.detail.is_some() {
        detail::render(frame, app, layout);
    }

    if app.filter_modal.is_some() {
        filter::render(frame, app, layout);
    }

    if app.modal.is_some() {
        view::render(frame, app, layout);
    }
}
