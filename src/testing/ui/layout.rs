use super::{Rect, home_recent_row_at, list_scroll_offset};

#[test]
fn list_scroll_offset_keeps_selected_row_visible() {
    let area = Rect::new(0, 0, 20, 9);

    assert_eq!(list_scroll_offset(area, 0, 12), 0);
    assert_eq!(list_scroll_offset(area, 6, 12), 0);
    assert_eq!(list_scroll_offset(area, 7, 12), 1);
    assert_eq!(list_scroll_offset(area, 11, 12), 5);
}

#[test]
fn home_recent_row_at_applies_scroll_offset() {
    let area = Rect::new(0, 0, 20, 9);

    assert_eq!(home_recent_row_at(area, 1, 1, 8, 12), Some(2));
    assert_eq!(home_recent_row_at(area, 1, 7, 8, 12), Some(8));
}
