use super::compact_query;

#[test]
fn compact_query_truncates_unicode_without_panicking() {
    let query = "SELECT '😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀'";
    let compact = compact_query(query);

    assert_eq!(compact, "SELECT '😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀...");
}

#[test]
fn compact_query_keeps_short_queries_unchanged() {
    assert_eq!(compact_query("SELECT name"), "SELECT name");
}

#[test]
fn compact_query_returns_placeholder_for_empty_input() {
    assert_eq!(compact_query(""), "<empty>");
    assert_eq!(compact_query("   "), "<empty>");
}

#[test]
fn compact_query_normalizes_multiline_queries() {
    assert_eq!(compact_query("SELECT\nname\nFROM demo"), "SELECT name FROM demo");
}

#[test]
fn compact_query_does_not_truncate_exact_limit() {
    assert_eq!(
        compact_query("12345678901234567890123456"),
        "12345678901234567890123456"
    );
}
