use std::collections::HashMap;

use anyhow::Result;

use super::cursor::previous_boundary;
use super::{App, SqlCompletionItem, SqlCompletionState, SqlPane};
use crate::db::TableSummary;

const SQL_KEYWORDS: &[&str] = &[
    "SELECT",
    "FROM",
    "WHERE",
    "ORDER BY",
    "GROUP BY",
    "LIMIT",
    "INSERT INTO",
    "VALUES",
    "UPDATE",
    "SET",
    "DELETE FROM",
    "CREATE TABLE",
    "ALTER TABLE",
    "DROP TABLE",
    "JOIN",
    "LEFT JOIN",
    "INNER JOIN",
    "PRAGMA",
];

const SQL_SNIPPETS: &[(&str, &str)] = &[
    ("SELECT * FROM", "SELECT *\nFROM "),
    ("SELECT WHERE", "SELECT *\nFROM \nWHERE "),
    ("INSERT INTO", "INSERT INTO  ()\nVALUES ();"),
    ("UPDATE SET", "UPDATE \nSET \nWHERE ;"),
    ("DELETE FROM", "DELETE FROM \nWHERE ;"),
];

impl App {
    pub(super) fn sql_refresh_completion(&mut self) -> Result<()> {
        if self.sql.focus != SqlPane::Editor {
            self.sql.completion = None;
            return Ok(());
        }

        let (prefix_start, prefix) = completion_prefix(&self.sql.query, self.sql.cursor);
        if prefix.is_empty() {
            self.sql.completion = None;
            return Ok(());
        }

        let items = self.sql_completion_candidates(&prefix)?;
        self.sql.completion = (!items.is_empty()).then_some(SqlCompletionState {
            prefix_start,
            selected: 0,
            items,
        });
        Ok(())
    }

    pub(super) fn sql_apply_completion(&mut self) {
        let Some(completion) = &self.sql.completion else {
            return;
        };
        let Some(item) = completion.items.get(completion.selected) else {
            return;
        };

        let end = self.sql.cursor;
        self.sql
            .query
            .replace_range(completion.prefix_start..end, &item.insert_text);
        self.sql.cursor = completion.prefix_start + item.insert_text.len();
        self.sql.completion = None;
        self.ensure_sql_viewport();
    }

    pub(super) fn sql_completion_candidates(
        &mut self,
        prefix: &str,
    ) -> Result<Vec<SqlCompletionItem>> {
        let prefix_lower = prefix.to_lowercase();
        let qualifier = completion_qualifier(prefix);
        let aliases = sql_aliases_before_cursor(&self.sql.query);
        let tables = completion_tables_for_qualifier(&self.tables, qualifier, &aliases)
            .into_iter()
            .map(|table| table.name.clone())
            .collect::<Vec<_>>();
        let use_full_table_prefix = has_ambiguous_bare_table_match(&self.tables, qualifier);
        let mut items = Vec::new();

        for keyword in SQL_KEYWORDS {
            items.push(SqlCompletionItem {
                label: (*keyword).to_string(),
                insert_text: (*keyword).to_string(),
            });
        }

        for (label, insert_text) in SQL_SNIPPETS {
            items.push(SqlCompletionItem {
                label: (*label).to_string(),
                insert_text: (*insert_text).to_string(),
            });
        }

        for table_name in &tables {
            let table_label = completion_table_label(self, table_name, qualifier);
            let table_insert_text = completion_table_insert_text(self, table_name, qualifier);
            items.push(SqlCompletionItem {
                label: table_label,
                insert_text: table_insert_text,
            });
        }

        let mut matches = filter_completion_items(items, &prefix_lower);
        if qualifier.is_empty() && matches.len() >= 6 {
            return Ok(matches);
        }

        let mut column_items = Vec::new();
        for table_name in tables {
            let table_label = completion_table_label(self, &table_name, qualifier);
            let insert_prefix =
                completion_insert_prefix(qualifier, &table_name, use_full_table_prefix);
            for column in self.sql_list_columns_cached(&table_name)? {
                column_items.push(SqlCompletionItem {
                    label: format!("{table_label}.{}", column),
                    insert_text: format!("{insert_prefix}{column}"),
                });
            }
        }

        matches.extend(filter_completion_items(column_items, &prefix_lower));
        matches.sort_by(|left, right| {
            left.label
                .cmp(&right.label)
                .then_with(|| left.insert_text.cmp(&right.insert_text))
        });
        matches.dedup_by(|left, right| {
            left.label.eq_ignore_ascii_case(&right.label) && left.insert_text == right.insert_text
        });
        matches.truncate(6);
        Ok(matches)
    }

    fn sql_list_columns_cached(&mut self, table_name: &str) -> Result<Vec<String>> {
        if let Some(columns) = self.sql.column_cache.get(table_name) {
            return Ok(columns.clone());
        }

        let columns = self.db_ref()?.list_columns(table_name)?;
        self.sql
            .column_cache
            .insert(table_name.to_string(), columns.clone());
        Ok(columns)
    }
}

fn filter_completion_items(
    mut items: Vec<SqlCompletionItem>,
    prefix_lower: &str,
) -> Vec<SqlCompletionItem> {
    items.sort_by(|left, right| {
        left.label
            .cmp(&right.label)
            .then_with(|| left.insert_text.cmp(&right.insert_text))
    });
    items.dedup_by(|left, right| {
        left.label.eq_ignore_ascii_case(&right.label) && left.insert_text == right.insert_text
    });
    items
        .into_iter()
        .filter(|item| completion_matches(prefix_lower, item))
        .take(6)
        .collect()
}

fn completion_matches(prefix_lower: &str, item: &SqlCompletionItem) -> bool {
    completion_text_matches(&item.label, prefix_lower)
        || completion_text_matches(&item.insert_text, prefix_lower)
}

fn completion_text_matches(text: &str, prefix_lower: &str) -> bool {
    let lower = text.to_lowercase();
    lower.starts_with(prefix_lower)
        || lower
            .split_once('.')
            .is_some_and(|(_, stripped)| stripped.starts_with(prefix_lower))
}

pub(super) fn completion_prefix(value: &str, cursor: usize) -> (usize, String) {
    let mut start = cursor;
    while start > 0 {
        let previous = previous_boundary(value, start);
        let ch = value[previous..start].chars().next().unwrap_or(' ');
        if is_completion_char(ch) {
            start = previous;
        } else {
            break;
        }
    }
    (start, value[start..cursor].to_string())
}

pub(super) fn completion_qualifier(prefix: &str) -> &str {
    prefix
        .rfind('.')
        .map(|index| &prefix[..=index])
        .unwrap_or("")
}

pub(super) fn completion_table_label(app: &App, table_name: &str, typed_qualifier: &str) -> String {
    if typed_qualifier.is_empty() {
        app.display_table_name(table_name)
    } else {
        table_name.to_string()
    }
}

pub(super) fn completion_table_insert_text(
    app: &App,
    table_name: &str,
    typed_qualifier: &str,
) -> String {
    if typed_qualifier.is_empty() {
        app.display_table_name(table_name)
    } else {
        table_name.to_string()
    }
}

pub(super) fn completion_insert_prefix(
    typed_qualifier: &str,
    table_name: &str,
    use_full_table_prefix: bool,
) -> String {
    if typed_qualifier.is_empty() {
        return String::new();
    }

    if use_full_table_prefix {
        return format!("{table_name}.");
    }

    let qualifier = typed_qualifier.trim_end_matches('.');
    if table_name.eq_ignore_ascii_case(qualifier)
        || table_name
            .rsplit('.')
            .next()
            .is_some_and(|bare_name| bare_name.eq_ignore_ascii_case(qualifier))
    {
        return typed_qualifier.to_string();
    }

    if table_name
        .split_once('.')
        .is_some_and(|(schema, _)| schema.eq_ignore_ascii_case(qualifier))
    {
        return format!("{table_name}.");
    }

    typed_qualifier.to_string()
}

fn sql_aliases_before_cursor(query: &str) -> HashMap<String, String> {
    let tokens = sql_tokens(query);
    let mut aliases = HashMap::new();
    let mut index = 0;

    while index < tokens.len() {
        let is_source_keyword = identifier_of(tokens.get(index))
            .map(|token| {
                matches!(
                    token.to_ascii_uppercase().as_str(),
                    "FROM" | "JOIN" | "UPDATE" | "INTO"
                )
            })
            .unwrap_or(false);
        if !is_source_keyword {
            index += 1;
            continue;
        }

        let Some((table_name, next_index)) = parse_table_reference(&tokens, index + 1) else {
            index += 1;
            continue;
        };

        index = next_index;
        let has_as = identifier_of(tokens.get(index))
            .map(|token| token.eq_ignore_ascii_case("AS"))
            .unwrap_or(false);
        if has_as {
            index += 1;
        }

        if let Some(alias) = identifier_of(tokens.get(index))
            && !is_clause_keyword(alias)
        {
            aliases.insert(alias.to_ascii_lowercase(), table_name);
            index += 1;
        }
    }

    aliases
}

fn sql_tokens(query: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();

    for ch in query.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            current.push(ch);
            continue;
        }

        if !current.is_empty() {
            tokens.push(std::mem::take(&mut current));
        }

        if !ch.is_whitespace() && matches!(ch, '.' | ',' | '(' | ')') {
            tokens.push(ch.to_string());
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

fn parse_table_reference(tokens: &[String], start: usize) -> Option<(String, usize)> {
    let first = identifier_of(tokens.get(start))?;
    let mut table_name = first.to_string();
    let mut index = start + 1;

    while index + 1 < tokens.len() && tokens[index] == "." {
        let next = identifier_of(tokens.get(index + 1))?;
        table_name.push('.');
        table_name.push_str(next);
        index += 2;
    }

    Some((table_name, index))
}

fn identifier_of(token: Option<&String>) -> Option<&str> {
    token.map(String::as_str).filter(|token| {
        token
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    })
}

fn is_clause_keyword(token: &str) -> bool {
    matches!(
        token.to_ascii_uppercase().as_str(),
        "ON" | "WHERE"
            | "GROUP"
            | "ORDER"
            | "LIMIT"
            | "LEFT"
            | "RIGHT"
            | "INNER"
            | "OUTER"
            | "JOIN"
    )
}

pub(super) fn completion_tables_for_qualifier<'a>(
    tables: &'a [TableSummary],
    qualifier: &str,
    aliases: &HashMap<String, String>,
) -> Vec<&'a TableSummary> {
    if qualifier.is_empty() {
        return tables.iter().collect();
    }

    let qualifier = qualifier.trim_end_matches('.');
    if let Some(target_table) = aliases.get(&qualifier.to_ascii_lowercase()) {
        return tables
            .iter()
            .filter(|table| {
                table.name.eq_ignore_ascii_case(target_table)
                    || table
                        .name
                        .rsplit('.')
                        .next()
                        .is_some_and(|name| name.eq_ignore_ascii_case(target_table))
            })
            .collect();
    }

    let exact_matches = tables
        .iter()
        .filter(|table| table.name.eq_ignore_ascii_case(qualifier))
        .collect::<Vec<_>>();
    if !exact_matches.is_empty() {
        return exact_matches;
    }

    let schema_matches = tables
        .iter()
        .filter(|table| {
            table
                .name
                .split_once('.')
                .is_some_and(|(schema, _)| schema.eq_ignore_ascii_case(qualifier))
        })
        .collect::<Vec<_>>();
    if !schema_matches.is_empty() {
        return schema_matches;
    }

    let bare_matches = tables
        .iter()
        .filter(|table| {
            table
                .name
                .rsplit('.')
                .next()
                .is_some_and(|name| name.eq_ignore_ascii_case(qualifier))
        })
        .collect::<Vec<_>>();
    if !bare_matches.is_empty() {
        return bare_matches;
    }

    Vec::new()
}

fn has_ambiguous_bare_table_match(tables: &[TableSummary], qualifier: &str) -> bool {
    let qualifier = qualifier.trim_end_matches('.');
    if qualifier.is_empty() || qualifier.contains('.') {
        return false;
    }

    tables
        .iter()
        .filter(|table| {
            table
                .name
                .rsplit('.')
                .next()
                .is_some_and(|name| name.eq_ignore_ascii_case(qualifier))
        })
        .take(2)
        .count()
        > 1
}

fn is_completion_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_' || ch == '.'
}

#[cfg(test)]
mod tests;
