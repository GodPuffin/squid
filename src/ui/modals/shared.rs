use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph};

use crate::theme::Theme;
use crate::ui::widgets::panel_block;

#[allow(clippy::too_many_arguments)]
pub fn render_shell(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    header: &str,
    footer: &str,
    header_area: Rect,
    footer_area: Rect,
    theme: Theme,
) {
    frame.render_widget(Clear, area);
    if theme.background.is_some() {
        frame.render_widget(Block::default().style(theme.fill_style()), area);
    }
    frame.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(theme.overlay_border_style())
            .style(theme.fill_style()),
        area,
    );
    frame.render_widget(
        Paragraph::new(header)
            .alignment(Alignment::Center)
            .style(theme.fg_style()),
        header_area,
    );
    frame.render_widget(
        Paragraph::new(footer)
            .alignment(Alignment::Center)
            .style(theme.muted_style()),
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
    theme: Theme,
) {
    let list_items: Vec<ListItem<'_>> = if items.is_empty() {
        vec![ListItem::new("No items")]
    } else {
        items.iter().cloned().map(ListItem::new).collect()
    };

    let list = List::new(list_items)
        .block(panel_block(title, focused, theme))
        .highlight_style(theme.selection_style())
        .highlight_symbol(">> ");

    let mut state = ListState::default();
    if !items.is_empty() {
        state.select(selected);
    }

    frame.render_stateful_widget(list, area, &mut state);
}
