use rusqlite::types::ValueRef;

#[test]
fn format_value_covers_all_value_types() {
    assert_eq!(super::format_value(ValueRef::Null), "NULL");
    assert_eq!(super::format_value(ValueRef::Integer(42)), "42");
    assert_eq!(super::format_value(ValueRef::Real(3.5)), "3.5");
    assert_eq!(super::format_value(ValueRef::Text(b"hello")), "hello");
    assert_eq!(super::format_value(ValueRef::Blob(&[1, 2, 3])), "<3 bytes>");
}
