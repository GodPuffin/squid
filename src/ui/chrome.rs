use ratatui::Frame;
use ratatui::layout::Alignment;
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::app::App;

pub fn render_header(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let (title, block_title) = if let Some(path) = app.path() {
        (format!("squid  {}", path.display()), "Database")
    } else {
        ("squid".to_string(), "Home")
    };
    let header = Paragraph::new(title)
        .block(Block::default().borders(Borders::ALL).title(block_title))
        .wrap(Wrap { trim: true });
    frame.render_widget(header, area);
}

pub fn render_footer(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let footer = Paragraph::new(app.footer_hint())
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::Gray));
    frame.render_widget(footer, area);
}
