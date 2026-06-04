use rusqlite::types::ValueRef;

#[test]
fn format_value_covers_all_value_types() {
    assert_eq!(super::format_value(ValueRef::Null), "NULL");
    assert_eq!(super::format_value(ValueRef::Integer(42)), "42");
    assert_eq!(super::format_value(ValueRef::Real(3.5)), "3.5");
    assert_eq!(super::format_value(ValueRef::Text(b"hello")), "hello");
    assert_eq!(super::format_value(ValueRef::Blob(&[1, 2, 3])), "<3 bytes>");
}

#[test]
fn format_detail_value_shows_blob_hex_and_utf8() {
    let text = b"hello";
    let rendered = super::format_detail_value(ValueRef::Blob(text));
    assert!(rendered.contains("<5 bytes>"));
    assert!(rendered.contains("Hex:"));
    assert!(rendered.contains("68 65 6c 6c 6f"));
    assert!(rendered.contains("UTF-8:"));
    assert!(rendered.contains("hello"));
}

#[test]
fn format_detail_value_omits_utf8_for_binary_blobs() {
    let rendered = super::format_detail_value(ValueRef::Blob(&[0xff, 0xfe, 0xfd]));
    assert!(rendered.contains("Hex:"));
    assert!(!rendered.contains("UTF-8:"));
}

#[test]
fn format_blob_hex_lines_groups_sixteen_bytes_per_line() {
    let bytes = (0u8..20).collect::<Vec<_>>();
    let lines = super::format_blob_hex_lines(&bytes);
    assert_eq!(lines.len(), 2);
    assert!(lines[0].starts_with("0000  "));
    assert!(lines[1].starts_with("0010  "));
}

#[test]
fn format_blob_detail_truncates_large_blobs() {
    let bytes = vec![0u8; super::BLOB_DETAIL_MAX_BYTES + 10];
    let rendered = super::format_detail_value(ValueRef::Blob(&bytes));
    assert!(rendered.contains(&format!("<{} bytes>", bytes.len())));
    assert!(rendered.contains("showing first"));
}
