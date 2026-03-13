use ratatui::Frame;

use crate::app::{App, ModalPane};

use super::shared::{render_list, render_shell};
use crate::ui::LayoutInfo;

pub fn render(frame: &mut Frame, app: &App, layout: &LayoutInfo) {
    let Some(modal_layout) = &layout.modal else {
        return;
    };

    render_shell(
        frame,
        modal_layout.area,
        "Configure View",
        "Hide columns and set sort order",
        "Space toggles | Enter adds current candidate | Delete removes active sort | c clears all",
        modal_layout.header,
        modal_layout.footer,
    );

    let (columns_idx, sort_columns_idx, sort_active_idx) = app.modal_selected_indices();

    render_list(
        frame,
        modal_layout.columns,
        "Columns",
        &app.modal_column_lines(),
        columns_idx,
        app.modal_pane() == Some(ModalPane::Columns),
    );
    render_list(
        frame,
        modal_layout.sort_candidates,
        "Sort Candidates",
        &app.modal_sort_column_lines(),
        sort_columns_idx,
        app.modal_pane() == Some(ModalPane::SortColumns),
    );
    render_list(
        frame,
        modal_layout.sort_stack,
        "Sort Stack",
        &app.modal_sort_active_lines(),
        sort_active_idx,
        app.modal_pane() == Some(ModalPane::SortActive),
    );
}
