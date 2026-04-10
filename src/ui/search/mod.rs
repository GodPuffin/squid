use ratatui::Frame;
use ratatui::layout::Constraint;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, List, ListItem, ListState, Paragraph, Row, Table, TableState, Wrap};

use crate::app::{App, PaneFocus, SearchScope};

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
                spans.extend(highlight_spans(&hit.haystack, &search.query));
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
                hit.values.iter().enumerate().map(|(idx, value)| {
                    if hit.matched_columns.get(idx).copied().unwrap_or(false) {
                        Cell::from(value.as_str()).style(
                            Style::default()
                                .fg(Color::LightYellow)
                                .add_modifier(Modifier::BOLD),
                        )
                    } else {
                        Cell::from(value.as_str())
                    }
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

fn highlight_spans<'a>(haystack: &'a str, query: &str) -> Vec<Span<'a>> {
    if query.is_empty() {
        return vec![Span::raw(haystack)];
    }

    let positions = fuzzy_match_positions(haystack, query);
    if positions.is_empty() {
        return vec![Span::raw(haystack)];
    }

    let mut spans = Vec::new();
    let mut normal_start = 0usize;

    for (char_idx, (byte_start, ch)) in haystack.char_indices().enumerate() {
        if positions.binary_search(&char_idx).is_ok() {
            if normal_start < byte_start {
                spans.push(Span::raw(&haystack[normal_start..byte_start]));
            }
            let byte_end = byte_start + ch.len_utf8();
            spans.push(Span::styled(
                &haystack[byte_start..byte_end],
                Style::default()
                    .fg(Color::LightYellow)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            ));
            normal_start = byte_end;
        }
    }

    if normal_start < haystack.len() {
        spans.push(Span::raw(&haystack[normal_start..]));
    }

    spans
}

fn fuzzy_match_positions(haystack: &str, query: &str) -> Vec<usize> {
    let haystack_lower: Vec<char> = haystack.to_lowercase().chars().collect();
    let query_lower: Vec<char> = query.to_lowercase().chars().collect();
    let mut positions = Vec::new();
    let mut search_index = 0_usize;

    for needle in query_lower {
        let mut found = None;
        for (idx, candidate) in haystack_lower.iter().enumerate().skip(search_index) {
            if *candidate == needle {
                found = Some(idx);
                break;
            }
        }
        if let Some(idx) = found {
            positions.push(idx);
            search_index = idx + 1;
        } else {
            return Vec::new();
        }
    }

    positions
}

#[cfg(test)]
mod tests;
