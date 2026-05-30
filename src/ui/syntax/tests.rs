use crate::theme::{ColorScheme, Theme};

#[test]
fn highlight_sql_line_styles_keywords_literals_and_comments() {
    let theme = Theme::from_scheme(ColorScheme::Dark);
    let spans = super::highlight_sql_line("SELECT name, 42, 'ok' -- note", theme);

    assert!(spans.iter().any(|span| {
        span.content.as_ref() == "SELECT" && span.style == theme.syntax_keyword_style()
    }));
    assert!(spans.iter().any(|span| {
        span.content.as_ref() == "42" && span.style == theme.syntax_number_style()
    }));
    assert!(spans.iter().any(|span| {
        span.content.as_ref() == "'ok'" && span.style == theme.syntax_string_style()
    }));
    assert!(spans.iter().any(|span| {
        span.content.as_ref() == "-- note" && span.style == theme.syntax_comment_style()
    }));
}

#[test]
fn highlight_sql_line_keeps_empty_lines_renderable() {
    let theme = Theme::from_scheme(ColorScheme::Dark);
    let spans = super::highlight_sql_line("", theme);

    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].content.as_ref(), "");
}
