use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders};

pub fn panel_block(title: &str, active: bool) -> Block<'_> {
    let border_style = if active {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    Block::default()
        .borders(Borders::ALL)
        .title(title.to_string())
        .border_style(border_style)
}
