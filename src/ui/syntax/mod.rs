use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;

pub(crate) fn highlight_sql_line(line: &str) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let chars = line.chars().collect::<Vec<_>>();
    let mut index = 0;

    while index < chars.len() {
        let ch = chars[index];
        if ch == '-' && chars.get(index + 1) == Some(&'-') {
            spans.push(Span::styled(
                chars[index..].iter().collect::<String>(),
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
                chars[start..index].iter().collect::<String>(),
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
                chars[start..index].iter().collect::<String>(),
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
            let token = chars[start..index].iter().collect::<String>();
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

        spans.push(Span::raw(ch.to_string()));
        index += 1;
    }

    if spans.is_empty() {
        vec![Span::raw(String::new())]
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
