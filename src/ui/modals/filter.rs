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
    let theme = app.theme();

    render_shell(
        frame,
        filter_layout.area,
        "Filters",
        "",
        &app.footer_hint(),
        filter_layout.header,
        filter_layout.footer,
        theme,
    );

    let (column_idx, mode_idx, active_idx) = app.filter_modal_selected_indices();

    render_list(
        frame,
        filter_layout.columns,
        "Columns",
        &app.modal_column_lines(),
        column_idx,
        app.filter_modal_pane() == Some(FilterPane::Columns),
        theme,
    );
    render_list(
        frame,
        filter_layout.modes,
        "Operators",
        &app.filter_modal_mode_lines(),
        mode_idx,
        app.filter_modal_pane() == Some(FilterPane::Modes),
        theme,
    );
    render_filter_workspace(
        frame,
        filter_layout.draft,
        app,
        app.filter_modal_pane() == Some(FilterPane::Draft),
        theme,
    );
    render_list(
        frame,
        filter_layout.active,
        "Active Filters",
        &app.filter_modal_active_lines(),
        active_idx,
        app.filter_modal_pane() == Some(FilterPane::Active),
        theme,
    );
}

fn render_filter_workspace(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    app: &App,
    focused: bool,
    theme: crate::theme::Theme,
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

    frame.render_widget(
        Paragraph::new(lines)
            .block(panel_block("Draft", focused, theme))
            .wrap(Wrap { trim: false })
            .style(theme.fg_style()),
        area,
    );
}
