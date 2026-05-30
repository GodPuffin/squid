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

    let visible_rows = chunks[1].height.saturating_sub(2) as usize;
    let scroll_offset = app.settings_scroll_offset(visible_rows);

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
            ListItem::new(Line::from(vec![
                Span::styled(format!("{label:<42}"), style),
                Span::styled(value, style),
            ]))
        })
        .collect();

    let mut state = ListState::default().with_offset(scroll_offset);
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
