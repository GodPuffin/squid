use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState, Paragraph, Wrap};

use crate::app::{App, DetailPane};

use super::shared::{render_shell, selection_style};
use crate::ui::LayoutInfo;
use crate::ui::widgets::panel_block;

const SAVE_LABEL: &str = " Save ";
const DISCARD_LABEL: &str = " Discard ";

pub struct DetailActionRects {
    pub header_save: Rect,
    pub header_discard: Rect,
}

pub fn render(frame: &mut Frame, app: &App, layout: &LayoutInfo) {
    let Some(detail_layout) = &layout.detail else {
        return;
    };
    let Some(detail) = &app.detail else {
        return;
    };

    render_shell(
        frame,
        detail_layout.area,
        "Row Details",
        "",
        "",
        detail_layout.header,
        detail_layout.footer,
    );

    render_header_bar(frame, app, detail_layout.header);

    let items: Vec<ListItem<'_>> = detail
        .fields
        .iter()
        .map(|field| {
            let style = if field.is_blob {
                Style::default().fg(Color::DarkGray)
            } else if field.is_dirty() {
                Style::default()
                    .fg(Color::LightGreen)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            let suffix = if field.is_blob {
                " [blob]"
            } else if field.foreign_target.is_some() {
                " ->"
            } else {
                ""
            };
            ListItem::new(Line::from(Span::styled(
                format!("{}{}", field.column_name, suffix),
                style,
            )))
        })
        .collect();
    let list = List::new(items)
        .block(panel_block(
            "Columns",
            detail.pane == DetailPane::Fields && !detail.is_editing,
        ))
        .highlight_style(selection_style())
        .highlight_symbol(">> ");
    let mut state = ListState::default();
    if !detail.fields.is_empty() {
        state.select(Some(detail.selected_field));
    }
    frame.render_stateful_widget(list, detail_layout.fields, &mut state);

    let (value_lines, value_title) = detail_value_content(app);
    let value = Paragraph::new(value_lines)
        .block(panel_block(
            &value_title,
            detail.pane == DetailPane::Value || detail.is_editing,
        ))
        .wrap(Wrap { trim: false })
        .scroll((detail.value_scroll as u16, 0))
        .style(Style::default());
    frame.render_widget(value, detail_layout.value);

    render_footer_bar(frame, app, detail_layout.footer);
}

pub fn action_rects(header: Rect, footer: Rect) -> DetailActionRects {
    let header_y = header.y.saturating_add(1);
    let header_discard = Rect::new(
        header.x + header.width.saturating_sub(DISCARD_LABEL.len() as u16 + 1),
        header_y,
        DISCARD_LABEL.len() as u16,
        1,
    );
    let header_save = Rect::new(
        header_discard.x.saturating_sub(SAVE_LABEL.len() as u16 + 1),
        header_y,
        SAVE_LABEL.len() as u16,
        1,
    );
    let _ = footer;

    DetailActionRects {
        header_save,
        header_discard,
    }
}

fn render_header_bar(frame: &mut Frame, app: &App, area: Rect) {
    let Some(detail) = &app.detail else {
        return;
    };
    let buttons = action_rects(area, area);
    let bar_area = Rect::new(
        area.x.saturating_add(1),
        area.y.saturating_add(1),
        area.width.saturating_sub(2),
        1,
    );
    let text_area = if app.detail_has_changes() {
        Rect::new(
            bar_area.x,
            bar_area.y,
            buttons
                .header_save
                .x
                .saturating_sub(bar_area.x)
                .saturating_sub(1),
            1,
        )
    } else {
        bar_area
    };

    let mut spans = vec![
        Span::styled(
            app.selected_table_label()
                .unwrap_or_else(|| "Row".to_string()),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(detail.row_label.clone(), Style::default().fg(Color::Gray)),
    ];

    if let Some(message) = &detail.message {
        spans.push(Span::raw("  "));
        spans.push(Span::styled(
            message.text.clone(),
            Style::default()
                .fg(if message.is_error {
                    Color::LightRed
                } else {
                    Color::LightGreen
                })
                .add_modifier(Modifier::BOLD),
        ));
    } else if app.detail_has_changes() {
        spans.push(Span::raw("  "));
        spans.push(Span::styled(
            format!(
                "{} pending edit(s)",
                detail
                    .fields
                    .iter()
                    .filter(|field| field.is_dirty())
                    .count()
            ),
            Style::default()
                .fg(Color::LightYellow)
                .add_modifier(Modifier::BOLD),
        ));
    }

    frame.render_widget(Paragraph::new(Line::from(spans)), text_area);

    if app.detail_has_changes() {
        frame.render_widget(
            Paragraph::new(Line::from(render_button("Save", Color::Green)))
                .alignment(Alignment::Right),
            buttons.header_save,
        );
        frame.render_widget(
            Paragraph::new(Line::from(render_button("Discard", Color::Red)))
                .alignment(Alignment::Right),
            buttons.header_discard,
        );
    }
}

fn render_footer_bar(frame: &mut Frame, app: &App, area: Rect) {
    let text = if app.detail_has_changes() {
        if app.detail_is_editing() {
            "Esc stop editing, then use Save or Discard above"
        } else {
            "Pending row changes  Save or Discard above"
        }
    } else if app.detail_is_editing() {
        "Type to edit  Enter newline  Backspace delete  Esc stop editing"
    } else if app.detail_is_row_writable() {
        "e edit field  Up/Down field  Left/Right pane  g follow foreign key"
    } else {
        "Read-only row  Up/Down field  Left/Right pane  g follow foreign key"
    };

    frame.render_widget(
        Paragraph::new(text)
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::Gray)),
        area,
    );
}

fn detail_value_content(app: &App) -> (Vec<Line<'static>>, String) {
    let Some(detail) = &app.detail else {
        return (vec![Line::from("")], "Value".to_string());
    };
    let Some(field) = detail.fields.get(detail.selected_field) else {
        return (vec![Line::from("No field selected")], "Value".to_string());
    };

    let mut title = if let Some(target) = &field.foreign_target {
        format!(
            "{} -> {}.{}",
            field.column_name,
            app.display_table_name(&target.table_name),
            target.column_name
        )
    } else {
        field.column_name.clone()
    };
    if detail.is_editing {
        title.push_str(" [editing]");
    } else if field.is_dirty() {
        title.push_str(" [edited]");
    } else if field.is_blob || detail.rowid.is_none() {
        title.push_str(" [read-only]");
    }

    let mut lines = vec![Line::from(Span::styled(
        format!(
            "Type: {}{}",
            if field.data_type.is_empty() {
                "TEXT"
            } else {
                field.data_type.as_str()
            },
            if field.not_null { "  NOT NULL" } else { "" }
        ),
        Style::default()
            .fg(Color::Gray)
            .add_modifier(Modifier::BOLD),
    ))];

    if field.is_blob {
        lines.push(Line::from(Span::styled(
            "Blob values are visible but not editable in the details modal.",
            Style::default().fg(Color::Gray),
        )));
    } else if detail.rowid.is_none() {
        lines.push(Line::from(Span::styled(
            "This row is read-only because rowid is unavailable.",
            Style::default().fg(Color::Gray),
        )));
    }
    lines.push(Line::from(""));

    if detail.is_editing || field.is_dirty() {
        lines.push(Line::from(Span::styled(
            "Original",
            Style::default()
                .fg(Color::Gray)
                .add_modifier(Modifier::BOLD),
        )));
        push_value_lines(
            &mut lines,
            &field.original_value,
            Style::default().fg(Color::Gray),
        );
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Draft",
            Style::default()
                .fg(Color::LightGreen)
                .add_modifier(Modifier::BOLD),
        )));
        push_value_lines(
            &mut lines,
            &field.draft_value,
            Style::default().fg(Color::LightGreen),
        );
    } else {
        push_value_lines(&mut lines, &field.draft_value, Style::default());
    }

    (lines, title)
}

fn push_value_lines(lines: &mut Vec<Line<'static>>, value: &str, style: Style) {
    if value.is_empty() {
        lines.push(Line::from(Span::styled(
            "<empty>".to_string(),
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        )));
        return;
    }

    for line in value.lines() {
        lines.push(Line::from(Span::styled(line.to_string(), style)));
    }
}

fn render_button<'a>(label: &'a str, color: Color) -> Span<'a> {
    Span::styled(
        format!(" {label} "),
        Style::default()
            .fg(Color::Black)
            .bg(color)
            .add_modifier(Modifier::BOLD),
    )
}
