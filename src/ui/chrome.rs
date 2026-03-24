use ratatui::Frame;
use ratatui::layout::Alignment;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::app::{App, AppMode};
use super::LayoutInfo;

pub fn render_header(frame: &mut Frame, app: &App, layout: &LayoutInfo) {
    let header = Block::default().borders(Borders::ALL).title("Database");
    frame.render_widget(header, layout.header);

    frame.render_widget(
        Paragraph::new(Line::from(render_tab(
            "1 Browse",
            app.mode == AppMode::Browse,
        ))),
        layout.header_tabs.browse,
    );
    frame.render_widget(
        Paragraph::new(Line::from(render_tab("2 SQL", app.mode == AppMode::Sql))),
        layout.header_tabs.sql,
    );
    frame.render_widget(
        Paragraph::new(
            app.path()
                .map(|path| path.display().to_string())
                .unwrap_or_default(),
        )
        .style(Style::default().fg(Color::Gray)),
        layout.header_tabs.path,
    );

    if app.mode == AppMode::Sql {
        frame.render_widget(
            Paragraph::new(Line::from(render_button("Run", Color::Green)))
                .alignment(Alignment::Right),
            layout.header_tabs.run,
        );
    }
    frame.render_widget(
        Paragraph::new(Line::from(render_button("Quit", Color::Red))).alignment(Alignment::Right),
        layout.header_tabs.quit,
    );
}

pub fn render_footer(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let footer = Paragraph::new(app.footer_hint())
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::Gray));
    frame.render_widget(footer, area);
}

fn render_tab<'a>(label: &'a str, active: bool) -> ratatui::text::Span<'a> {
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
    ratatui::text::Span::styled(format!(" {label} "), style)
}

fn render_button<'a>(label: &'a str, color: Color) -> ratatui::text::Span<'a> {
    ratatui::text::Span::styled(
        format!(" {label} "),
        Style::default()
            .fg(Color::Black)
            .bg(color)
            .add_modifier(Modifier::BOLD),
    )
}
