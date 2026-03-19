use ratatui::Frame;
use ratatui::layout::Alignment;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::app::{App, AppMode};

use super::LayoutInfo;

pub fn render_header(frame: &mut Frame, app: &App, layout: &LayoutInfo) {
    let tabs = vec![
        render_tab("1 Browse", app.mode == AppMode::Browse),
        Span::raw(" "),
        render_tab("2 SQL", app.mode == AppMode::Sql),
        Span::raw("   "),
        Span::styled(
            app.path().display().to_string(),
            Style::default().fg(Color::Gray),
        ),
    ];
    let header = Paragraph::new(Line::from(tabs))
        .block(Block::default().borders(Borders::ALL).title("Database"))
        .alignment(Alignment::Left);
    frame.render_widget(header, layout.header);
}

pub fn render_footer(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let footer = Paragraph::new(app.footer_hint())
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::Gray));
    frame.render_widget(footer, area);
}

fn render_tab<'a>(label: &'a str, active: bool) -> Span<'a> {
    let style = if active {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(Color::White)
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD)
    };
    Span::styled(format!(" {label} "), style)
}
