use ratatui::Frame;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::{List, ListItem, ListState, Paragraph, Wrap};

use crate::app::{App, DetailPane};

use super::shared::{render_shell, selection_style};
use crate::ui::LayoutInfo;
use crate::ui::widgets::panel_block;

pub fn render(frame: &mut Frame, app: &App, layout: &LayoutInfo) {
    let Some(detail_layout) = &layout.detail else {
        return;
    };
    let Some(detail) = &app.detail else {
        return;
    };

    let header = format!(
        "{}  {}",
        app.selected_table_label().as_deref().unwrap_or("Row"),
        detail.row_label
    );
    let (_value_title, value_lines, helper, value_title) = detail_value_content(app);

    render_shell(
        frame,
        detail_layout.area,
        "Row Details",
        &header,
        &helper,
        detail_layout.header,
        detail_layout.footer,
    );

    let items: Vec<ListItem<'_>> = app
        .detail_field_lines()
        .into_iter()
        .map(ListItem::new)
        .collect();
    let list = List::new(items)
        .block(panel_block("Columns", detail.pane == DetailPane::Fields))
        .highlight_style(selection_style())
        .highlight_symbol(">> ");
    let mut state = ListState::default();
    if !detail.fields.is_empty() {
        state.select(Some(detail.selected_field));
    }
    frame.render_stateful_widget(list, detail_layout.fields, &mut state);

    let value = Paragraph::new(value_lines)
        .block(panel_block(&value_title, detail.pane == DetailPane::Value))
        .wrap(Wrap { trim: false })
        .scroll((detail.value_scroll as u16, 0))
        .style(Style::default());
    frame.render_widget(value, detail_layout.value);
}

fn detail_value_content(app: &App) -> (String, Vec<Line<'static>>, String, String) {
    let Some(detail) = &app.detail else {
        return (
            "Value".to_string(),
            vec![Line::from("")],
            String::new(),
            "Value".to_string(),
        );
    };
    let Some(field) = detail.fields.get(detail.selected_field) else {
        return (
            "Value".to_string(),
            vec![Line::from("No field selected")],
            String::new(),
            "Value".to_string(),
        );
    };

    let title = if let Some(target) = &field.foreign_target {
        format!(
            "{}  -> {}.{}",
            field.column_name,
            app.display_table_name(&target.table_name),
            target.column_name
        )
    } else {
        field.column_name.clone()
    };
    let helper = if field.foreign_target.is_some() {
        "g follows the referenced row"
    } else {
        "Use Left/Right to switch between field list and full value"
    };
    let lines = field
        .value
        .lines()
        .map(|line| Line::from(line.to_string()))
        .collect();

    (title.clone(), lines, helper.to_string(), title)
}
