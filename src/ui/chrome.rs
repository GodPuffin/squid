use ratatui::Frame;
use ratatui::layout::Alignment;
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::app::App;

pub fn render_header(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let title = format!("squid  {}", app.path().display());
    let header = Paragraph::new(title)
        .block(Block::default().borders(Borders::ALL).title("Database"))
        .wrap(Wrap { trim: true });
    frame.render_widget(header, area);
}

pub fn render_footer(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let footer = Paragraph::new(app.footer_hint())
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::Gray));
    frame.render_widget(footer, area);
}
