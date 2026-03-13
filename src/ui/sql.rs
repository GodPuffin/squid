use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Cell, Clear, List, ListItem, ListState, Paragraph, Row, Table, Wrap};

use crate::app::{App, SqlPane, SqlResultState};

use super::LayoutInfo;
use super::layout::sql_completion_rect;
use super::modals::shared::selection_style;
use super::widgets::panel_block;

pub fn render(frame: &mut Frame, app: &App, layout: &LayoutInfo) {
    let Some(sql) = &layout.sql else {
        return;
    };

    render_editor(frame, app, sql.editor);
    render_history(frame, app, sql.history);
    render_results(frame, app, sql.results);
    render_completion(frame, app, sql.editor, sql.completion);
}

fn render_editor(frame: &mut Frame, app: &App, area: Rect) {
    let lines = app.sql_query_lines();
    let visible = lines
        .iter()
        .skip(app.sql.editor_scroll)
        .take(app.sql.editor_height)
        .map(|line| Line::from(highlight_sql_line(line)))
        .collect::<Vec<_>>();

    let text = if visible.is_empty() {
        Text::from(Line::from(vec![Span::styled(
            "-- Write SQL here. Press F5 to run, F2 for completion.",
            Style::default().fg(Color::DarkGray),
        )]))
    } else {
        Text::from(visible)
    };

    let editor = Paragraph::new(text)
        .block(panel_block(
            "SQL Editor",
            app.sql_focus() == SqlPane::Editor,
        ))
        .wrap(Wrap { trim: false });
    frame.render_widget(editor, area);

    if app.sql_focus() == SqlPane::Editor {
        let inner_x = area.x.saturating_add(1);
        let inner_y = area.y.saturating_add(1);
        let width = area.width.saturating_sub(2) as usize;
        let (line, col) = app.sql_cursor_line_col();
        let cursor_y = line.saturating_sub(app.sql.editor_scroll);
        if cursor_y < app.sql.editor_height && col < width {
            frame.set_cursor_position((
                inner_x.saturating_add(col as u16),
                inner_y.saturating_add(cursor_y as u16),
            ));
        }
    }
}

fn render_history(frame: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem<'_>> = if app.sql.history.is_empty() {
        vec![ListItem::new("No query history")]
    } else {
        app.sql_visible_history()
            .iter()
            .map(|entry| {
                ListItem::new(format!(
                    "{}  {}",
                    compact_query(&entry.query),
                    entry.summary
                ))
            })
            .collect()
    };

    let history = List::new(items)
        .block(panel_block("History", app.sql_focus() == SqlPane::History))
        .highlight_style(selection_style())
        .highlight_symbol(">> ");

    let mut state = ListState::default();
    state.select(app.sql_selected_history_in_view());
    frame.render_stateful_widget(history, area, &mut state);
}

fn render_results(frame: &mut Frame, app: &App, area: Rect) {
    match &app.sql.result {
        SqlResultState::Empty => {
            let empty = Paragraph::new("Run a query to see results")
                .block(panel_block("Results", app.sql_focus() == SqlPane::Results))
                .wrap(Wrap { trim: true });
            frame.render_widget(empty, area);
        }
        SqlResultState::Message { text, is_error } => {
            let style = if *is_error {
                Style::default()
                    .fg(Color::LightRed)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::LightGreen)
            };
            let message = Paragraph::new(Line::from(Span::styled(text.clone(), style)))
                .block(panel_block("Results", app.sql_focus() == SqlPane::Results))
                .wrap(Wrap { trim: true });
            frame.render_widget(message, area);
        }
        SqlResultState::Rows { .. } => render_result_table(frame, app, area),
    }
}

fn render_result_table(frame: &mut Frame, app: &App, area: Rect) {
    let columns = app.sql_result_columns();
    let rows = app.sql_result_rows_in_view();
    let widths = columns
        .iter()
        .map(|_| Constraint::Min(12))
        .collect::<Vec<_>>();
    let header =
        Row::new(columns.iter().cloned()).style(Style::default().add_modifier(Modifier::BOLD));
    let body = rows
        .iter()
        .map(|row| Row::new(row.iter().cloned().map(Cell::from)));
    let table = Table::new(body, widths)
        .header(header)
        .block(panel_block("Results", app.sql_focus() == SqlPane::Results))
        .column_spacing(1);
    frame.render_widget(table, area);
}

fn render_completion(frame: &mut Frame, app: &App, editor_area: Rect, popup_rect: Option<Rect>) {
    let items = app.sql_completion_items();
    if items.is_empty() {
        return;
    }

    let popup = popup_rect.unwrap_or_else(|| {
        let (line, col) = app.sql_cursor_line_col();
        sql_completion_rect(editor_area, line.saturating_sub(app.sql.editor_scroll), col)
    });

    frame.render_widget(Clear, popup);
    let list = List::new(
        items
            .iter()
            .take(6)
            .map(|item| ListItem::new(item.label.clone()))
            .collect::<Vec<_>>(),
    )
    .block(panel_block("Completion", true))
    .highlight_style(selection_style())
    .highlight_symbol(">> ");

    let mut state = ListState::default();
    state.select(app.sql_selected_completion_in_view());
    frame.render_stateful_widget(list, popup, &mut state);
}

fn highlight_sql_line(line: &str) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let chars = line.chars().collect::<Vec<_>>();
    let mut index = 0;

    while index < chars.len() {
        let ch = chars[index];
        if ch == '-' && chars.get(index + 1) == Some(&'-') {
            spans.push(Span::styled(
                chars[index..].iter().collect::<String>(),
                Style::default().fg(Color::DarkGray),
            ));
            break;
        }
        if ch == '\'' {
            let start = index;
            index += 1;
            while index < chars.len() {
                if chars[index] == '\'' {
                    index += 1;
                    break;
                }
                index += 1;
            }
            spans.push(Span::styled(
                chars[start..index].iter().collect::<String>(),
                Style::default().fg(Color::LightGreen),
            ));
            continue;
        }
        if ch.is_ascii_digit() {
            let start = index;
            index += 1;
            while index < chars.len() && chars[index].is_ascii_digit() {
                index += 1;
            }
            spans.push(Span::styled(
                chars[start..index].iter().collect::<String>(),
                Style::default().fg(Color::LightMagenta),
            ));
            continue;
        }
        if ch.is_ascii_alphabetic() || ch == '_' {
            let start = index;
            index += 1;
            while index < chars.len()
                && (chars[index].is_ascii_alphanumeric() || chars[index] == '_')
            {
                index += 1;
            }
            let token = chars[start..index].iter().collect::<String>();
            let upper = token.to_uppercase();
            let style = if is_sql_keyword(&upper) {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            spans.push(Span::styled(token, style));
            continue;
        }

        spans.push(Span::raw(ch.to_string()));
        index += 1;
    }

    if spans.is_empty() {
        vec![Span::raw(String::new())]
    } else {
        spans
    }
}

fn is_sql_keyword(token: &str) -> bool {
    matches!(
        token,
        "SELECT"
            | "FROM"
            | "WHERE"
            | "ORDER"
            | "BY"
            | "GROUP"
            | "LIMIT"
            | "INSERT"
            | "INTO"
            | "VALUES"
            | "UPDATE"
            | "SET"
            | "DELETE"
            | "CREATE"
            | "TABLE"
            | "ALTER"
            | "DROP"
            | "JOIN"
            | "LEFT"
            | "INNER"
            | "PRAGMA"
            | "AS"
            | "AND"
            | "OR"
    )
}

fn compact_query(query: &str) -> String {
    let single_line = query.replace('\n', " ");
    let compact = single_line.trim();
    if compact.len() > 26 {
        format!("{}...", &compact[..26])
    } else if compact.is_empty() {
        "<empty>".to_string()
    } else {
        compact.to_string()
    }
}
