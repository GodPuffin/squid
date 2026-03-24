#[test]
fn fuzzy_match_positions_returns_ordered_character_matches() {
    assert_eq!(
        super::fuzzy_match_positions("Alphabet", "abt"),
        vec![0, 5, 7]
    );
}

#[test]
fn fuzzy_match_positions_returns_empty_when_query_cannot_match() {
    assert!(super::fuzzy_match_positions("abc", "adz").is_empty());
}

#[test]
fn highlight_spans_marks_only_matching_positions() {
    let spans = super::highlight_spans("abc", "ac");
    assert_eq!(spans.len(), 3);
    assert_eq!(spans[0].content.as_ref(), "a");
    assert_eq!(spans[1].content.as_ref(), "b");
    assert_eq!(spans[2].content.as_ref(), "c");
    assert_ne!(spans[0].style, spans[1].style);
    assert_ne!(spans[2].style, spans[1].style);
}
