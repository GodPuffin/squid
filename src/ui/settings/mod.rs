use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};

use crate::app::App;

use super::LayoutInfo;
use super::widgets::panel_block;

pub fn render(frame: &mut Frame, app: &App, layout: &LayoutInfo) {
    let area = centered_rect(layout.content, 72, 80);
    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(6),
        Constraint::Length(3),
    ])
    .split(area);

    let title = Paragraph::new("Settings")
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .alignment(Alignment::Center)
        .style(Style::default().add_modifier(Modifier::BOLD));
    frame.render_widget(title, chunks[0]);

    let row_width = chunks[1].width.saturating_sub(2) as usize;
    let items: Vec<ListItem<'_>> = app
        .settings_lines()
        .into_iter()
        .map(|(label, value, selected)| {
            let style = if selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(settings_row_line(&label, &value, row_width, style))
        })
        .collect();

    let mut state = ListState::default();
    if let Some(selected) = app.settings_selected_index() {
        state.select(Some(selected));
    }

    let list = List::new(items)
        .block(panel_block("preferences", true))
        .highlight_symbol("");
    frame.render_stateful_widget(list, chunks[1], &mut state);

    let description = Paragraph::new(app.settings_selected_description())
        .wrap(Wrap { trim: true })
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(description, chunks[2]);

    let hint = Paragraph::new(app.settings_footer_hint())
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(hint, layout.footer);
}

fn settings_row_line(label: &str, value: &str, width: usize, style: Style) -> Line<'static> {
    const MIN_GAP: usize = 2;
    let value_width = value.chars().count();
    let mut label_text = label.to_string();
    let max_label_width = width.saturating_sub(value_width).saturating_sub(MIN_GAP);
    if label_text.chars().count() > max_label_width {
        label_text = truncate_chars(&label_text, max_label_width.saturating_sub(1));
        label_text.push('…');
    }
    let label_width = label_text.chars().count();
    let gap = width
        .saturating_sub(label_width)
        .saturating_sub(value_width);
    Line::from(vec![
        Span::styled(label_text, style),
        Span::styled(" ".repeat(gap), style),
        Span::styled(value.to_string(), style),
    ])
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}

fn centered_rect(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let popup_layout = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(area);

    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(popup_layout[1])[1]
}

#[cfg(test)]
mod tests {
    use ratatui::style::Style;
    use ratatui::text::Span;

    use super::{settings_row_line, truncate_chars};

    fn line_width(line: &ratatui::text::Line<'_>) -> usize {
        line.spans
            .iter()
            .map(|span: &Span<'_>| span.content.chars().count())
            .sum()
    }

    fn line_value(line: &ratatui::text::Line<'_>) -> String {
        line.spans.last().expect("value span").content.to_string()
    }

    #[test]
    fn settings_row_line_right_aligns_value_within_width() {
        let line = settings_row_line("Mouse support", "on", 60, Style::default());
        assert_eq!(line_width(&line), 60);
        assert_eq!(line_value(&line), "on");
        assert!(line.spans[1].content.chars().all(|ch| ch == ' '));
    }

    #[test]
    fn settings_row_line_truncates_long_labels_before_value() {
        let label = "Restore session when reopening a database";
        let line = settings_row_line(label, "off", 40, Style::default());
        assert_eq!(line_width(&line), 40);
        assert_eq!(line_value(&line), "off");
    }

    #[test]
    fn truncate_chars_limits_by_character_count() {
        assert_eq!(truncate_chars("abcdef", 4), "abcd");
    }
}
