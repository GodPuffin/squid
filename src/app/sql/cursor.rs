pub(super) fn previous_boundary(value: &str, index: usize) -> usize {
    value[..index]
        .char_indices()
        .last()
        .map(|(idx, _)| idx)
        .unwrap_or(0)
}

pub(super) fn next_boundary(value: &str, index: usize) -> usize {
    value[index..]
        .char_indices()
        .nth(1)
        .map(|(offset, _)| index + offset)
        .unwrap_or_else(|| value.len())
}

pub(super) fn line_col_from_index(value: &str, index: usize) -> (usize, usize) {
    let mut line = 0;
    let mut col = 0;
    for (byte_idx, ch) in value.char_indices() {
        if byte_idx >= index {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    (line, col)
}

pub(super) fn index_for_line_col(value: &str, target_line: usize, target_col: usize) -> usize {
    let mut line = 0;
    let mut col = 0;
    for (idx, ch) in value.char_indices() {
        if line == target_line && col == target_col {
            return idx;
        }
        if ch == '\n' {
            if line == target_line {
                return idx;
            }
            line += 1;
            col = 0;
        } else if line == target_line {
            col += 1;
        }
    }
    value.len()
}

pub(super) fn split_lines(value: &str) -> Vec<String> {
    if value.is_empty() {
        vec![String::new()]
    } else {
        value.split('\n').map(str::to_string).collect()
    }
}

pub(super) fn line_length(value: &str, target_line: usize) -> usize {
    split_lines(value)
        .into_iter()
        .nth(target_line)
        .map(|line| line.chars().count())
        .unwrap_or(0)
}

pub(super) fn move_vertical(value: &str, index: usize, delta: isize) -> usize {
    let (line, col) = line_col_from_index(value, index);
    let target_line = if delta.is_negative() {
        line.saturating_sub(delta.unsigned_abs())
    } else {
        line.saturating_add(delta as usize)
    };
    let target_col = col.min(line_length(value, target_line));
    index_for_line_col(value, target_line, target_col)
}

#[cfg(test)]
mod tests {
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

        assert_eq!(split_lines(query), vec!["SELECT".to_string(), String::new()]);
        assert_eq!(line_length(query, 1), 0);
    }
}
