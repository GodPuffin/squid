#[test]
fn fuzzy_match_positions_returns_ordered_character_matches() {
    assert_eq!(
        super::fuzzy_match_positions("Alphabet", "abt"),
        vec![4, 5, 7]
    );
}

#[test]
fn fuzzy_match_positions_returns_empty_when_query_cannot_match() {
    assert!(super::fuzzy_match_positions("abc", "adz").is_empty());
}

#[test]
fn highlight_spans_marks_only_matching_positions() {
    let spans = super::highlight_fuzzy_spans("abc", "ac");
    assert_eq!(spans.len(), 3);
    assert_eq!(spans[0].content.as_ref(), "a");
    assert_eq!(spans[1].content.as_ref(), "b");
    assert_eq!(spans[2].content.as_ref(), "c");
    assert_ne!(spans[0].style, spans[1].style);
    assert_ne!(spans[2].style, spans[1].style);
}

#[test]
fn highlight_exact_spans_uses_contiguous_match() {
    let spans = super::highlight_exact_spans("a-b-abc", "abc");
    assert_eq!(spans.len(), 2);
    assert_eq!(spans[0].content.as_ref(), "a-b-");
    assert_eq!(spans[1].content.as_ref(), "abc");
    assert_ne!(spans[0].style, spans[1].style);
}

#[test]
fn current_table_highlight_prefers_exact_match_when_available() {
    let spans = super::highlight_current_table_value_spans("A Canadian drama", "canadian");
    assert_eq!(spans.len(), 3);
    assert_eq!(spans[0].content.as_ref(), "A ");
    assert_eq!(spans[1].content.as_ref(), "Canadian");
    assert_eq!(spans[2].content.as_ref(), " drama");
    assert_ne!(spans[0].style, spans[1].style);
}

#[test]
fn current_table_empty_message_handles_empty_query() {
    assert_eq!(
        super::current_table_empty_message("", true),
        "Type to filter current table"
    );
    assert_eq!(
        super::current_table_empty_message("", false),
        "Press Enter to search current table"
    );
}

#[test]
fn current_table_empty_message_handles_non_empty_query() {
    assert_eq!(
        super::current_table_empty_message("needle", true),
        "No matches"
    );
    assert_eq!(
        super::current_table_empty_message("needle", false),
        "Press Enter to search current table"
    );
}

#[test]
fn search_loading_message_matches_scope() {
    assert_eq!(
        super::search_loading_message(crate::app::SearchScope::CurrentTable),
        "Searching current table exhaustively..."
    );
    assert_eq!(
        super::search_loading_message(crate::app::SearchScope::AllTables),
        "Searching all tables exhaustively..."
    );
}

#[test]
fn crop_spans_skips_prefix_characters_across_spans() {
    let spans = super::crop_spans(
        vec![
            ratatui::text::Span::raw("ab"),
            ratatui::text::Span::styled("cd", super::search_highlight_style()),
            ratatui::text::Span::raw("ef"),
        ],
        3,
    );

    assert_eq!(spans.len(), 2);
    assert_eq!(spans[0].content.as_ref(), "d");
    assert_eq!(spans[1].content.as_ref(), "ef");
    assert_eq!(spans[0].style, super::search_highlight_style());
}
