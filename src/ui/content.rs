use ratatui::Frame;
use ratatui::layout::Constraint;
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, Row, Table, TableState, Wrap};

use crate::app::{App, ContentView, PaneFocus};

use super::LayoutInfo;
use super::search::render_search;
use super::widgets::panel_block;

pub fn render(frame: &mut Frame, app: &App, layout: &LayoutInfo) {
    if app.is_home() {
        render_home(frame, app, layout.content);
        return;
    }

    match app.content_view {
        ContentView::Rows => render_rows(frame, app, layout),
        ContentView::Schema => render_schema(frame, app, layout.content),
    }
}

fn render_home(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let lines: Vec<Line<'_>> = app.schema_lines().into_iter().map(Line::from).collect();
    let home = Paragraph::new(lines)
        .block(panel_block("Welcome", app.focus == PaneFocus::Content))
        .wrap(Wrap { trim: false });
    frame.render_widget(home, area);
}

fn render_rows(frame: &mut Frame, app: &App, layout: &LayoutInfo) {
    if app.search.is_some() {
        render_search(frame, app, layout);
        return;
    }

    let title = app.content_title();

    if app.preview.columns.is_empty() {
        let message = Paragraph::new("No rows to preview")
            .block(panel_block(&title, app.focus == PaneFocus::Content))
            .wrap(Wrap { trim: true });
        frame.render_widget(message, layout.content);
        return;
    }

    let widths: Vec<Constraint> = app
        .preview
        .columns
        .iter()
        .map(|_| Constraint::Min(12))
        .collect();

    let header =
        Row::new(std::iter::once("#".to_string()).chain(app.preview.columns.iter().cloned()))
            .style(Style::default().add_modifier(Modifier::BOLD));

    let rows = app.preview.rows.iter().enumerate().map(|(idx, row)| {
        let row_number = app.row_offset + idx + 1;
        Row::new(std::iter::once(row_number.to_string()).chain(row.iter().cloned()))
    });

    let mut all_widths = vec![Constraint::Length(6)];
    all_widths.extend(widths);

    let table = Table::new(rows, all_widths)
        .header(header)
        .block(panel_block(&title, app.focus == PaneFocus::Content))
        .row_highlight_style(super::modals::shared::selection_style())
        .highlight_symbol(">> ")
        .column_spacing(1);

    let mut state = TableState::default();
    state.select(app.selected_row_in_view());
    frame.render_stateful_widget(table, layout.content, &mut state);
}

fn render_schema(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let title = app.content_title();
    let lines: Vec<Line<'_>> = app.schema_lines().into_iter().map(Line::from).collect();

    let schema = Paragraph::new(lines)
        .block(panel_block(&title, app.focus == PaneFocus::Content))
        .scroll((app.schema_offset as u16, 0))
        .wrap(Wrap { trim: false });

    frame.render_widget(schema, area);
}
