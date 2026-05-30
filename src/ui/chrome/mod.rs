use ratatui::Frame;
use ratatui::layout::Alignment;
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Paragraph};

use super::LayoutInfo;
use crate::app::{App, AppMode};
use crate::theme::Theme;

pub fn render_header(frame: &mut Frame, app: &App, layout: &LayoutInfo) {
    let theme = app.theme();
    let header = Block::default()
        .borders(Borders::ALL)
        .title("Database")
        .border_style(theme.overlay_border_style())
        .style(theme.fill_style());
    frame.render_widget(header, layout.header);

    frame.render_widget(
        Paragraph::new(Line::from(render_tab(
            "1 Browse",
            app.mode == AppMode::Browse,
            theme,
        ))),
        layout.header_tabs.browse,
    );
    frame.render_widget(
        Paragraph::new(Line::from(render_tab(
            "2 SQL",
            app.mode == AppMode::Sql,
            theme,
        ))),
        layout.header_tabs.sql,
    );
    frame.render_widget(
        Paragraph::new(
            app.path()
                .map(|path| path.display().to_string())
                .unwrap_or_default(),
        )
        .style(theme.muted_style()),
        layout.header_tabs.path,
    );

    if app.mode == AppMode::Sql {
        frame.render_widget(
            Paragraph::new(Line::from(render_button("Run", theme.success, theme)))
                .alignment(Alignment::Right),
            layout.header_tabs.run,
        );
    }
    frame.render_widget(
        Paragraph::new(Line::from(render_button("Quit", theme.error, theme)))
            .alignment(Alignment::Right),
        layout.header_tabs.quit,
    );
}

pub fn render_footer(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let theme = app.theme();
    let footer = Paragraph::new(app.footer_hint())
        .alignment(Alignment::Center)
        .style(theme.muted_style());
    frame.render_widget(footer, area);
}

fn render_tab<'a>(label: &'a str, active: bool, theme: Theme) -> ratatui::text::Span<'a> {
    ratatui::text::Span::styled(format!(" {label} "), theme.tab_style(active))
}

fn render_button<'a>(
    label: &'a str,
    color: ratatui::style::Color,
    theme: Theme,
) -> ratatui::text::Span<'a> {
    ratatui::text::Span::styled(format!(" {label} "), theme.button_style(color))
}
