use ratatui::style::{Color, Modifier, Style};

#[test]
fn highlight_sql_line_styles_keywords_literals_and_comments() {
    let spans = super::highlight_sql_line("SELECT name, 42, 'ok' -- note");

    assert!(spans.iter().any(|span| {
        span.content.as_ref() == "SELECT"
            && span.style
                == Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
    }));
    assert!(spans.iter().any(|span| {
        span.content.as_ref() == "42" && span.style == Style::default().fg(Color::LightMagenta)
    }));
    assert!(spans.iter().any(|span| {
        span.content.as_ref() == "'ok'" && span.style == Style::default().fg(Color::LightGreen)
    }));
    assert!(spans.iter().any(|span| {
        span.content.as_ref() == "-- note" && span.style == Style::default().fg(Color::DarkGray)
    }));
}

#[test]
fn highlight_sql_line_keeps_empty_lines_renderable() {
    let spans = super::highlight_sql_line("");

    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].content.as_ref(), "");
}
