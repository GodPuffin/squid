use super::{line_col_from_index, line_length, move_vertical, split_lines};

#[test]
fn line_column_round_trips() {
    let query = "SELECT\nname";
    assert_eq!(line_col_from_index(query, 0), (0, 0));
    assert_eq!(line_col_from_index(query, 7), (1, 0));
}

#[test]
fn vertical_movement_preserves_column_when_possible() {
    let query = "SELECT\ncolumn\nx";
    let moved = move_vertical(query, query.len() - 1, -1);
    assert_eq!(line_col_from_index(query, moved), (1, 0));
}

#[test]
fn split_lines_preserves_trailing_empty_line() {
    let query = "SELECT\n";

    assert_eq!(
        split_lines(query),
        vec!["SELECT".to_string(), String::new()]
    );
    assert_eq!(line_length(query, 1), 0);
}
