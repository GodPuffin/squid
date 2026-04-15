use ratatui::Frame;
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, Wrap};

use crate::app::{App, FilterPane};
use crate::db::FilterMode;

use super::shared::{render_list, render_shell};
use crate::ui::LayoutInfo;
use crate::ui::widgets::panel_block;

pub fn render(frame: &mut Frame, app: &App, layout: &LayoutInfo) {
    let Some(filter_layout) = &layout.filter_modal else {
        return;
    };

    render_shell(
        frame,
        filter_layout.area,
        "Filters",
        "Pick a column, choose an operator, and apply a row filter",
        "Space toggles columns, adds space in Draft, or cycles operator in Modes | Enter applies",
        filter_layout.header,
        filter_layout.footer,
    );

    let (column_idx, mode_idx, active_idx) = app.filter_modal_selected_indices();

    render_list(
        frame,
        filter_layout.columns,
        "Columns",
        &app.modal_column_lines(),
        column_idx,
        app.filter_modal_pane() == Some(FilterPane::Columns),
    );
    render_list(
        frame,
        filter_layout.modes,
        "Operators",
        &app.filter_modal_mode_lines(),
        mode_idx,
        app.filter_modal_pane() == Some(FilterPane::Modes),
    );
    render_filter_workspace(
        frame,
        filter_layout.draft,
        app,
        app.filter_modal_pane() == Some(FilterPane::Draft),
    );
    render_list(
        frame,
        filter_layout.active,
        "Active Filters",
        &app.filter_modal_active_lines(),
        active_idx,
        app.filter_modal_pane() == Some(FilterPane::Active),
    );
}

fn render_filter_workspace(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    app: &App,
    focused: bool,
) {
    let mode = match app.modal_filter_mode() {
        FilterMode::Contains => "contains",
        FilterMode::Equals => "equals",
        FilterMode::StartsWith => "starts with",
        FilterMode::GreaterThan => "greater than",
        FilterMode::LessThan => "less than",
        FilterMode::IsTrue => "is true",
        FilterMode::IsFalse => "is false",
    };
    let uses_input = !matches!(
        app.modal_filter_mode(),
        FilterMode::IsTrue | FilterMode::IsFalse
    );

    let mut lines = vec![
        Line::from(format!("Column: {}", app.modal_filter_column_name())),
        Line::from(format!("Mode:   {mode}")),
    ];
    if uses_input {
        lines.push(Line::from(format!("Value:  {}", app.modal_filter_input())));
    } else {
        lines.push(Line::from("Value:  none"));
    }
    lines.push(Line::from(""));
    lines.push(Line::from("Move focus here to type the filter value."));

    frame.render_widget(
        Paragraph::new(lines)
            .block(panel_block("Draft", focused))
            .wrap(Wrap { trim: false }),
        area,
    );
}
