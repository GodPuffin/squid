use ratatui::Frame;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{List, ListItem, ListState};

use crate::app::{App, PaneFocus};

use super::widgets::panel_block;

pub fn render_tables(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let (title, items): (&str, Vec<ListItem<'_>>) = if app.is_home() {
        if app.recent_items.is_empty() {
            ("Recent Files", vec![ListItem::new("No recent files")])
        } else {
            (
                "Recent Files",
                app.recent_items
                    .iter()
                    .map(|item| {
                        let mut label = item.path.display().to_string();
                        if !item.available {
                            label.push_str(" [missing]");
                        }
                        ListItem::new(label)
                    })
                    .collect(),
            )
        }
    } else if app.tables.is_empty() {
        ("Tables", vec![ListItem::new("No tables")])
    } else {
        (
            "Tables",
            app.tables
                .iter()
                .map(|table| ListItem::new(app.display_table_name(&table.name)))
                .collect(),
        )
    };

    let list = List::new(items)
        .block(panel_block(title, app.focus == PaneFocus::Tables))
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

    let mut state = ListState::default();
    if app.is_home() {
        if !app.recent_items.is_empty() {
            state.select(Some(app.selected_recent));
        }
    } else if !app.tables.is_empty() {
        state.select(Some(app.selected_table));
    }

    frame.render_stateful_widget(list, area, &mut state);
}
