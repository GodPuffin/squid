use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph};

use crate::ui::widgets::panel_block;

pub fn selection_style() -> Style {
    Style::default()
        .fg(Color::Black)
        .bg(Color::Yellow)
        .add_modifier(Modifier::BOLD)
}

pub fn overlay_border_style() -> Style {
    Style::default()
        .fg(Color::LightCyan)
        .add_modifier(Modifier::BOLD)
}

pub fn render_shell(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    header: &str,
    footer: &str,
    header_area: Rect,
    footer_area: Rect,
) {
    frame.render_widget(Clear, area);
    frame.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(overlay_border_style()),
        area,
    );
    frame.render_widget(
        Paragraph::new(header).alignment(Alignment::Center),
        header_area,
    );
    frame.render_widget(
        Paragraph::new(footer)
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::Gray)),
        footer_area,
    );
}

pub fn render_list(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    items: &[String],
    selected: Option<usize>,
    focused: bool,
) {
    let list_items: Vec<ListItem<'_>> = if items.is_empty() {
        vec![ListItem::new("No items")]
    } else {
        items.iter().cloned().map(ListItem::new).collect()
    };

    let list = List::new(list_items)
        .block(panel_block(title, focused))
        .highlight_style(selection_style())
        .highlight_symbol(">> ");

    let mut state = ListState::default();
    if !items.is_empty() {
        state.select(selected);
    }

    frame.render_stateful_widget(list, area, &mut state);
}
