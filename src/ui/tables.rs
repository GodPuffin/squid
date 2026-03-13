use ratatui::Frame;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{List, ListItem, ListState};

use crate::app::{App, PaneFocus};

use super::widgets::panel_block;

pub fn render_tables(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let items: Vec<ListItem<'_>> = if app.tables.is_empty() {
        vec![ListItem::new("No tables")]
    } else {
        app.tables
            .iter()
            .map(|table| ListItem::new(table.name.clone()))
            .collect()
    };

    let list = List::new(items)
        .block(panel_block("Tables", app.focus == PaneFocus::Tables))
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

    let mut state = ListState::default();
    if !app.tables.is_empty() {
        state.select(Some(app.selected_table));
    }

    frame.render_stateful_widget(list, area, &mut state);
}
