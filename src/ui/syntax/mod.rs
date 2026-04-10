use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;

pub(crate) fn highlight_sql_line(line: &str) -> Vec<Span<'_>> {
    let mut spans = Vec::new();
    let chars = line.chars().collect::<Vec<_>>();
    let mut byte_indices = line.char_indices().map(|(idx, _)| idx).collect::<Vec<_>>();
    byte_indices.push(line.len());
    let mut index = 0;

    while index < chars.len() {
        let ch = chars[index];
        if ch == '-' && chars.get(index + 1) == Some(&'-') {
            spans.push(Span::styled(
                &line[byte_indices[index]..],
                Style::default().fg(Color::DarkGray),
            ));
            break;
        }
        if ch == '\'' {
            let start = index;
            index += 1;
            while index < chars.len() {
                if chars[index] == '\'' {
                    index += 1;
                    break;
                }
                index += 1;
            }
            spans.push(Span::styled(
                &line[byte_indices[start]..byte_indices[index]],
                Style::default().fg(Color::LightGreen),
            ));
            continue;
        }
        if ch.is_ascii_digit() {
            let start = index;
            index += 1;
            while index < chars.len() && chars[index].is_ascii_digit() {
                index += 1;
            }
            spans.push(Span::styled(
                &line[byte_indices[start]..byte_indices[index]],
                Style::default().fg(Color::LightMagenta),
            ));
            continue;
        }
        if ch.is_ascii_alphabetic() || ch == '_' {
            let start = index;
            index += 1;
            while index < chars.len()
                && (chars[index].is_ascii_alphanumeric() || chars[index] == '_')
            {
                index += 1;
            }
            let token = &line[byte_indices[start]..byte_indices[index]];
            let upper = token.to_uppercase();
            let style = if is_sql_keyword(&upper) {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            spans.push(Span::styled(token, style));
            continue;
        }

        spans.push(Span::raw(
            &line[byte_indices[index]..byte_indices[index + 1]],
        ));
        index += 1;
    }

    if spans.is_empty() {
        vec![Span::raw("")]
    } else {
        spans
    }
}

fn is_sql_keyword(token: &str) -> bool {
    matches!(
        token,
        "SELECT"
            | "FROM"
            | "WHERE"
            | "ORDER"
            | "BY"
            | "GROUP"
            | "LIMIT"
            | "INSERT"
            | "INTO"
            | "VALUES"
            | "UPDATE"
            | "SET"
            | "DELETE"
            | "CREATE"
            | "TABLE"
            | "PRIMARY"
            | "KEY"
            | "NOT"
            | "NULL"
            | "DEFAULT"
            | "UNIQUE"
            | "CHECK"
            | "REFERENCES"
            | "FOREIGN"
            | "CONSTRAINT"
            | "INDEX"
            | "ALTER"
            | "DROP"
            | "JOIN"
            | "LEFT"
            | "INNER"
            | "PRAGMA"
            | "AS"
            | "AND"
            | "OR"
    )
}

#[cfg(test)]
mod tests;
