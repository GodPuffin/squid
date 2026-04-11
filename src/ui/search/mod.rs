use ratatui::Frame;
use ratatui::layout::Constraint;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, List, ListItem, ListState, Paragraph, Row, Table, TableState, Wrap};

use crate::app::{App, PaneFocus, SearchScope};
use crate::db::fuzzy_match_positions;

use super::modals::shared::selection_style;
use super::widgets::panel_block;
use super::{LayoutInfo, layout::search_layout};

pub fn render_search(frame: &mut Frame, app: &App, layout: &LayoutInfo) {
    let Some(search) = &app.search else {
        return;
    };
    let sections = search_layout(layout.content);
    let scope = match search.scope {
        SearchScope::CurrentTable => "Current Table",
        SearchScope::AllTables => "All Tables",
    };
    let search_title = format!("Search {scope}");

    let query = Paragraph::new(search.query.as_str())
        .block(panel_block(&search_title, app.focus == PaneFocus::Content))
        .wrap(Wrap { trim: false });
    frame.render_widget(query, sections[0]);

    if search.loading {
        let loading = Paragraph::new(search_loading_message(search.scope))
            .block(panel_block("Results", app.focus == PaneFocus::Content))
            .wrap(Wrap { trim: false });
        frame.render_widget(loading, sections[1]);
        return;
    }

    if matches!(search.scope, SearchScope::CurrentTable) {
        render_current_table_search(frame, app, sections[1]);
        return;
    }

    let visible_results = search
        .results
        .iter()
        .skip(search.result_offset)
        .take(search.result_limit)
        .collect::<Vec<_>>();

    let items: Vec<ListItem<'_>> = if visible_results.is_empty() && !search.submitted {
        vec![ListItem::new("Press Enter to search all tables")]
    } else if visible_results.is_empty() {
        vec![ListItem::new("No matches")]
    } else {
        visible_results
            .iter()
            .map(|hit| {
                let mut spans = vec![
                    Span::styled(
                        format!("{}  ", app.display_table_name(&hit.table_name)),
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(hit.row_label.as_str(), Style::default().fg(Color::Gray)),
                    Span::raw("  "),
                ];
                spans.extend(highlight_exact_spans(&hit.haystack, &search.query));
                ListItem::new(Line::from(spans))
            })
            .collect()
    };

    let list = List::new(items)
        .block(panel_block("Results", app.focus == PaneFocus::Content))
        .highlight_style(selection_style())
        .highlight_symbol(">> ");

    let mut state = ListState::default();
    state.select(app.search_selected_index_in_view());
    frame.render_stateful_widget(list, sections[1], &mut state);
}

fn render_current_table_search(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let Some(search) = &app.search else {
        return;
    };
    if search.results.is_empty() {
        let message = current_table_empty_message(search.query.as_str(), search.submitted);
        let empty = Paragraph::new(message)
            .block(panel_block("Results", app.focus == PaneFocus::Content))
            .wrap(Wrap { trim: false });
        frame.render_widget(empty, area);
        return;
    }

    let headers = app.search_headers();
    let widths: Vec<Constraint> = headers.iter().map(|_| Constraint::Min(12)).collect();
    let header = Row::new(std::iter::once("#").chain(headers.iter().map(String::as_str)))
        .style(Style::default().add_modifier(Modifier::BOLD));

    let rows = search
        .results
        .iter()
        .skip(search.result_offset)
        .take(search.result_limit)
        .map(|hit| {
            let styled_cells = std::iter::once(Cell::from(hit.row_label.as_str())).chain(
                hit.values.iter().map(|value| {
                    Cell::from(Line::from(highlight_current_table_value_spans(
                        value,
                        &search.query,
                    )))
                }),
            );
            Row::new(styled_cells)
        });

    let mut all_widths = vec![Constraint::Length(14)];
    all_widths.extend(widths);
    let table = Table::new(rows, all_widths)
        .header(header)
        .block(panel_block("Results", app.focus == PaneFocus::Content))
        .row_highlight_style(selection_style())
        .highlight_symbol(">> ")
        .column_spacing(1);

    let mut state = TableState::default();
    state.select(app.search_selected_index_in_view());
    frame.render_stateful_widget(table, area, &mut state);
}

fn current_table_empty_message(query: &str, submitted: bool) -> &'static str {
    if query.is_empty() {
        if submitted {
            "Type to filter current table"
        } else {
            "Press Enter to search current table"
        }
    } else if submitted {
        "No matches"
    } else {
        "Press Enter to search current table"
    }
}

fn search_loading_message(scope: SearchScope) -> &'static str {
    match scope {
        SearchScope::CurrentTable => "Searching current table exhaustively...",
        SearchScope::AllTables => "Searching all tables exhaustively...",
    }
}

fn highlight_current_table_value_spans<'a>(value: &'a str, query: &str) -> Vec<Span<'a>> {
    if exact_match_range(value, query).is_some() {
        highlight_exact_spans(value, query)
    } else {
        highlight_fuzzy_spans(value, query)
    }
}

fn highlight_fuzzy_spans<'a>(haystack: &'a str, query: &str) -> Vec<Span<'a>> {
    if query.is_empty() {
        return vec![Span::raw(haystack)];
    }

    let positions = fuzzy_match_positions(haystack, query);
    if positions.is_empty() {
        return vec![Span::raw(haystack)];
    }

    highlight_char_positions(haystack, &positions)
}

fn highlight_exact_spans<'a>(haystack: &'a str, query: &str) -> Vec<Span<'a>> {
    if query.is_empty() {
        return vec![Span::raw(haystack)];
    }

    let Some((start, end)) = exact_match_range(haystack, query) else {
        return vec![Span::raw(haystack)];
    };

    let start_byte = byte_offset_for_char_index(haystack, start);
    let end_byte = byte_offset_for_char_index(haystack, end);
    let mut spans = Vec::new();
    if start_byte > 0 {
        spans.push(Span::raw(&haystack[..start_byte]));
    }
    spans.push(Span::styled(
        &haystack[start_byte..end_byte],
        search_highlight_style(),
    ));
    if end_byte < haystack.len() {
        spans.push(Span::raw(&haystack[end_byte..]));
    }
    spans
}

fn highlight_char_positions<'a>(haystack: &'a str, positions: &[usize]) -> Vec<Span<'a>> {
    let mut spans = Vec::new();
    let mut normal_start = 0usize;

    for (char_idx, (byte_start, ch)) in haystack.char_indices().enumerate() {
        if positions.binary_search(&char_idx).is_ok() {
            if normal_start < byte_start {
                spans.push(Span::raw(&haystack[normal_start..byte_start]));
            }
            let byte_end = byte_start + ch.len_utf8();
            spans.push(Span::styled(&haystack[byte_start..byte_end], search_highlight_style()));
            normal_start = byte_end;
        }
    }

    if normal_start < haystack.len() {
        spans.push(Span::raw(&haystack[normal_start..]));
    }

    spans
}

fn exact_match_range(haystack: &str, query: &str) -> Option<(usize, usize)> {
    let haystack_lower: Vec<char> = haystack.to_lowercase().chars().collect();
    let query_lower: Vec<char> = query.to_lowercase().chars().collect();

    if query_lower.is_empty() || haystack_lower.len() < query_lower.len() {
        return None;
    }

    for start in 0..=haystack_lower.len() - query_lower.len() {
        if haystack_lower[start..start + query_lower.len()] == query_lower[..] {
            return Some((start, start + query_lower.len()));
        }
    }

    None
}
fn byte_offset_for_char_index(value: &str, char_index: usize) -> usize {
    value
        .char_indices()
        .nth(char_index)
        .map(|(byte_index, _)| byte_index)
        .unwrap_or(value.len())
}

fn search_highlight_style() -> Style {
    Style::default()
        .fg(Color::LightYellow)
        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
}

#[cfg(test)]
mod tests;
