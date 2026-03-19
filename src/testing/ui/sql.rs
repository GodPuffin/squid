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
