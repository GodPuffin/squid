use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, List, ListItem, ListState, Paragraph, Row, Table, TableState, Wrap};
use std::sync::{Mutex, OnceLock};

use crate::app::{App, ContentView, PaneFocus};
use crate::theme::Theme;

use super::LayoutInfo;
use super::search::render_search;
use super::syntax::highlight_sql_line;
use super::widgets::panel_block;

pub fn render(frame: &mut Frame, app: &App, layout: &LayoutInfo) {
    if app.is_home() {
        render_home(frame, app, layout);
        return;
    }

    match app.content_view {
        ContentView::Rows => render_rows(frame, app, layout),
        ContentView::Schema => render_schema(frame, app, layout.content),
    }
}

fn render_home(frame: &mut Frame, app: &App, layout: &LayoutInfo) {
    let theme = app.theme();
    let raw_logo_lines = app.home_logo_lines();
    let logo_width = raw_logo_lines
        .iter()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(0) as u16;
    let logo_height = raw_logo_lines.len() as u16;
    let logo_area = centered_fixed_rect(layout.header, logo_width, logo_height);
    let logo_lines: Vec<Line<'_>> = raw_logo_lines.into_iter().map(Line::from).collect();
    let logo = Paragraph::new(logo_lines).style(theme.fg_style());
    frame.render_widget(logo, logo_area);

    let items: Vec<ListItem<'_>> = app
        .home_recent_lines()
        .into_iter()
        .map(ListItem::new)
        .collect();
    let recents = List::new(items)
        .block(panel_block(
            "recents",
            app.focus == PaneFocus::Tables,
            theme,
        ))
        .highlight_style(theme.list_highlight_style())
        .highlight_symbol("  ");
    let mut state = ListState::default().with_offset(super::list_scroll_offset(
        layout.tables,
        app.selected_recent,
        app.recent_items.len(),
    ));
    if !app.recent_items.is_empty() {
        state.select(Some(app.selected_recent));
    }
    frame.render_stateful_widget(recents, layout.tables, &mut state);

    if let Some(status) = app.home_status_line() {
        let status = Paragraph::new(status)
            .alignment(Alignment::Center)
            .style(theme.fg_style());
        frame.render_widget(status, layout.content);
    }

    let controls = Paragraph::new(app.footer_hint())
        .alignment(Alignment::Center)
        .style(theme.muted_weak_style());
    frame.render_widget(controls, layout.footer);
}

fn centered_fixed_rect(
    area: ratatui::layout::Rect,
    width: u16,
    height: u16,
) -> ratatui::layout::Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    ratatui::layout::Rect::new(x, y, width, height)
}

fn render_rows(frame: &mut Frame, app: &App, layout: &LayoutInfo) {
    if app.search.is_some() {
        render_search(frame, app, layout);
        return;
    }

    let theme = app.theme();
    let title = app.content_title();

    if app.preview.columns.is_empty() {
        let message = Paragraph::new("No rows to preview")
            .block(panel_block(&title, app.focus == PaneFocus::Content, theme))
            .wrap(Wrap { trim: true })
            .style(theme.fg_style());
        frame.render_widget(message, layout.content);
        return;
    }

    let show_row_numbers = app.app_settings.show_row_numbers;

    let widths: Vec<Constraint> = app
        .preview
        .columns
        .iter()
        .map(|_| Constraint::Min(12))
        .collect();

    let header_cells = app
        .preview
        .columns
        .iter()
        .map(|column| Cell::from(column.as_str()));
    let header = if show_row_numbers {
        Row::new(std::iter::once(Cell::from("#")).chain(header_cells))
    } else {
        Row::new(header_cells)
    }
    .style(theme.emphasis_style());

    let rows = app.preview.rows.iter().enumerate().map(|(idx, row)| {
        let data_cells = row
            .iter()
            .map(|value| Cell::from(app.format_cell_preview(value)));
        if show_row_numbers {
            let row_number = app.row_offset + idx + 1;
            Row::new(std::iter::once(Cell::from(row_number.to_string())).chain(data_cells))
        } else {
            Row::new(data_cells)
        }
    });

    let mut all_widths = if show_row_numbers {
        vec![Constraint::Length(6)]
    } else {
        Vec::new()
    };
    all_widths.extend(widths);

    let table = Table::new(rows, all_widths)
        .header(header)
        .block(panel_block(&title, app.focus == PaneFocus::Content, theme))
        .row_highlight_style(theme.selection_style())
        .highlight_symbol(">> ")
        .column_spacing(1);

    let mut state = TableState::default();
    state.select(app.selected_row_in_view());
    frame.render_stateful_widget(table, layout.content, &mut state);
}

fn render_schema(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let theme = app.theme();
    let title = app.content_title();
    let lines = schema_display_lines(app, theme);

    let schema = Paragraph::new(lines)
        .block(panel_block(&title, app.focus == PaneFocus::Content, theme))
        .scroll((app.schema_offset as u16, 0))
        .wrap(Wrap { trim: false })
        .style(theme.fg_style());

    frame.render_widget(schema, area);
}

pub(super) fn schema_display_lines(app: &App, theme: Theme) -> Vec<Line<'static>> {
    static CACHE: OnceLock<Mutex<Option<(u64, Vec<Line<'static>>)>>> = OnceLock::new();
    let key = app.schema_cache_key();
    let cache = CACHE.get_or_init(|| Mutex::new(None));
    if let Ok(guard) = cache.lock()
        && let Some((cached_key, lines)) = guard.as_ref()
        && *cached_key == key
    {
        return lines.clone();
    }

    let mut in_create_sql = false;
    let lines = app
        .schema_lines()
        .into_iter()
        .map(|line| {
            if in_create_sql {
                return Line::from(
                    highlight_sql_line(&line, theme)
                        .into_iter()
                        .map(|span| Span::styled(span.content.into_owned(), span.style))
                        .collect::<Vec<_>>(),
                );
            }

            if line == "Create SQL" {
                in_create_sql = true;
            }

            Line::from(Span::styled(line, theme.fg_style()))
        })
        .collect::<Vec<_>>();

    if let Ok(mut guard) = cache.lock() {
        *guard = Some((key, lines.clone()));
    }

    lines
}

#[cfg(test)]
mod tests;
