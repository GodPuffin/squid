use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

use crate::app::App;

use super::layout::centered_rect;

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let theme = app.theme();
    let overlay = centered_rect(area, 52, 62);
    frame.render_widget(Clear, overlay);

    if theme.background.is_some() {
        frame.render_widget(Block::default().style(theme.fill_style()), overlay);
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .title(app.help_title())
        .border_style(theme.overlay_border_style())
        .style(theme.fill_style());
    let inner = block.inner(overlay);
    frame.render_widget(block, overlay);

    let rows = Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);

    let entries = app.help_entries();
    let lines: Vec<Line<'_>> = entries
        .iter()
        .map(|entry| {
            Line::from(vec![
                Span::styled(
                    format!("{:>14}  ", entry.key),
                    theme.emphasis_style(),
                ),
                Span::styled(entry.description.as_str(), theme.fg_style()),
            ])
        })
        .collect();

    let body = Paragraph::new(lines).wrap(Wrap { trim: true });
    frame.render_widget(body, rows[0]);

    let footer = Paragraph::new("Esc or ? close")
        .alignment(Alignment::Center)
        .style(theme.muted_weak_style());
    frame.render_widget(footer, rows[1]);
}
