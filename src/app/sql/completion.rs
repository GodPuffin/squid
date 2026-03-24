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

        let columns = self.db.list_columns(table_name)?;
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
            .map(|token| matches!(token.to_ascii_uppercase().as_str(), "FROM" | "JOIN" | "UPDATE" | "INTO"))
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

        if let Some(alias) = identifier_of(tokens.get(index)) {
            if !is_clause_keyword(alias) {
                aliases.insert(alias.to_ascii_lowercase(), table_name);
                index += 1;
            }
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
    token
        .map(String::as_str)
        .filter(|token| token.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '_'))
}

fn is_clause_keyword(token: &str) -> bool {
    matches!(
        token.to_ascii_uppercase().as_str(),
        "ON" | "WHERE" | "GROUP" | "ORDER" | "LIMIT" | "LEFT" | "RIGHT" | "INNER" | "OUTER" | "JOIN"
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
mod tests {
    use std::collections::HashMap;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use rusqlite::Connection;

    use super::{
        completion_insert_prefix, completion_prefix, completion_qualifier,
        completion_table_insert_text, completion_table_label, completion_tables_for_qualifier,
        sql_aliases_before_cursor,
    };
    use crate::app::App;

    #[test]
    fn completion_prefix_reads_identifier_prefix() {
        let query = "SELECT ac";
        let (start, prefix) = completion_prefix(query, query.len());
        assert_eq!(start, 7);
        assert_eq!(prefix, "ac");
    }

    #[test]
    fn completion_qualifier_keeps_table_or_alias_prefix() {
        assert_eq!(completion_qualifier("orders."), "orders.");
        assert_eq!(completion_qualifier("o.id"), "o.");
        assert_eq!(completion_qualifier("id"), "");
    }

    #[test]
    fn completion_insert_prefix_expands_schema_qualifiers_to_full_table_names() {
        assert_eq!(
            completion_insert_prefix("main.", "main.orders", false),
            "main.orders."
        );
        assert_eq!(
            completion_insert_prefix("orders.", "main.orders", false),
            "orders."
        );
        assert_eq!(completion_insert_prefix("o.", "main.orders", false), "o.");
    }

    #[test]
    fn completion_uses_bare_main_names_when_not_ambiguous() {
        let app = test_app_with_tables(
            "main-labels",
            &["CREATE TABLE orders(id INTEGER PRIMARY KEY)"],
        );
        assert_eq!(completion_table_label(&app, "main.orders", ""), "orders");
        assert_eq!(
            completion_table_insert_text(&app, "main.orders", ""),
            "orders"
        );
    }

    #[test]
    fn sql_completion_preserves_qualified_prefix_when_applied() {
        let path = temp_db_path("qualified-completion");
        let conn = Connection::open(&path).expect("create db");
        conn.execute("CREATE TABLE orders(id INTEGER PRIMARY KEY, name TEXT)", [])
            .expect("create table");
        drop(conn);

        let mut app = App::load(path.clone()).expect("load app");
        app.sql.query = "SELECT main.orders.".to_string();
        app.sql.cursor = app.sql.query.len();
        app.sql_refresh_completion().expect("refresh completion");
        let completion = app.sql.completion.as_mut().expect("completion");
        completion.selected = completion
            .items
            .iter()
            .position(|item| item.label == "main.orders.id")
            .expect("main.orders.id completion");

        app.sql_apply_completion();

        assert_eq!(app.sql.query, "SELECT main.orders.id");

        let _ = fs::remove_file(path);
    }

    #[test]
    fn sql_completion_qualified_table_filters_out_other_tables() {
        let path = temp_db_path("qualified-filter");
        let conn = Connection::open(&path).expect("create db");
        conn.execute("CREATE TABLE orders(id INTEGER PRIMARY KEY)", [])
            .expect("create orders");
        conn.execute("CREATE TABLE customers(name TEXT)", [])
            .expect("create customers");
        drop(conn);

        let mut app = App::load(path.clone()).expect("load app");
        app.sql.query = "SELECT main.orders.".to_string();
        app.sql.cursor = app.sql.query.len();
        app.sql_refresh_completion().expect("refresh completion");

        let labels = app
            .sql
            .completion
            .as_ref()
            .expect("completion")
            .items
            .iter()
            .map(|item| item.label.as_str())
            .collect::<Vec<_>>();

        assert!(labels.contains(&"main.orders.id"));
        assert!(!labels.contains(&"main.customers.name"));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn sql_completion_matches_alias_qualified_prefixes() {
        let path = temp_db_path("alias-completion");
        let conn = Connection::open(&path).expect("create db");
        conn.execute("CREATE TABLE orders(id INTEGER PRIMARY KEY, name TEXT)", [])
            .expect("create table");
        drop(conn);

        let mut app = App::load(path.clone()).expect("load app");
        app.sql.query = "SELECT o. FROM orders o".to_string();
        app.sql.cursor = "SELECT o.".len();
        app.sql_refresh_completion().expect("refresh completion");

        let items = app
            .sql
            .completion
            .as_ref()
            .expect("completion")
            .items
            .iter()
            .map(|item| item.insert_text.as_str())
            .collect::<Vec<_>>();

        assert!(items.contains(&"o.id"));
        assert!(items.contains(&"o.name"));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn sql_completion_does_not_offer_alias_columns_from_other_tables() {
        let path = temp_db_path("alias-filter");
        let conn = Connection::open(&path).expect("create db");
        conn.execute("CREATE TABLE orders(id INTEGER PRIMARY KEY)", [])
            .expect("create orders");
        conn.execute("CREATE TABLE users(name TEXT)", [])
            .expect("create users");
        drop(conn);

        let mut app = App::load(path.clone()).expect("load app");
        app.sql.query = "SELECT u. FROM orders o JOIN users u ON u.rowid = o.rowid".to_string();
        app.sql.cursor = "SELECT u.".len();
        app.sql_refresh_completion().expect("refresh completion");

        let items = app
            .sql
            .completion
            .as_ref()
            .expect("completion")
            .items
            .iter()
            .map(|item| item.insert_text.as_str())
            .collect::<Vec<_>>();

        assert!(items.contains(&"u.name"));
        assert!(!items.contains(&"u.id"));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn sql_completion_matches_unqualified_column_prefixes() {
        let path = temp_db_path("column-prefix-completion");
        let conn = Connection::open(&path).expect("create db");
        conn.execute("CREATE TABLE orders(id INTEGER PRIMARY KEY, name TEXT)", [])
            .expect("create table");
        drop(conn);

        let mut app = App::load(path.clone()).expect("load app");
        app.sql.query = "SELECT na".to_string();
        app.sql.cursor = app.sql.query.len();
        app.sql_refresh_completion().expect("refresh completion");

        let items = app
            .sql
            .completion
            .as_ref()
            .expect("completion")
            .items
            .iter()
            .map(|item| item.insert_text.as_str())
            .collect::<Vec<_>>();

        assert!(items.contains(&"name"));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn sql_completion_inserts_full_table_after_schema_qualifier() {
        let path = temp_db_path("schema-prefix-completion");
        let conn = Connection::open(&path).expect("create db");
        conn.execute("CREATE TABLE orders(id INTEGER PRIMARY KEY, name TEXT)", [])
            .expect("create table");
        drop(conn);

        let mut app = App::load(path.clone()).expect("load app");
        app.sql.query = "SELECT main.".to_string();
        app.sql.cursor = app.sql.query.len();
        app.sql_refresh_completion().expect("refresh completion");
        let completion = app.sql.completion.as_mut().expect("completion");
        completion.selected = completion
            .items
            .iter()
            .position(|item| item.label == "main.orders.id")
            .expect("main.orders.id completion");

        app.sql_apply_completion();

        assert_eq!(app.sql.query, "SELECT main.orders.id");

        let _ = fs::remove_file(path);
    }

    #[test]
    fn completion_tables_for_qualifier_only_narrows_on_matching_tables() {
        let tables = vec![
            crate::db::TableSummary {
                name: "main.orders".to_string(),
            },
            crate::db::TableSummary {
                name: "main.customers".to_string(),
            },
            crate::db::TableSummary {
                name: "temp.scratch".to_string(),
            },
        ];
        let aliases = HashMap::new();

        let narrowed = completion_tables_for_qualifier(&tables, "main.orders.", &aliases);
        assert_eq!(narrowed.len(), 1);
        assert_eq!(narrowed[0].name, "main.orders");

        let schema_only = completion_tables_for_qualifier(&tables, "temp.", &aliases);
        assert_eq!(schema_only.len(), 1);
        assert_eq!(schema_only[0].name, "temp.scratch");

        let alias_fallback = completion_tables_for_qualifier(&tables, "o.", &aliases);
        assert!(alias_fallback.is_empty());
    }

    #[test]
    fn completion_tables_for_ambiguous_bare_name_excludes_unrelated_tables() {
        let tables = vec![
            crate::db::TableSummary {
                name: "main.orders".to_string(),
            },
            crate::db::TableSummary {
                name: "other.orders".to_string(),
            },
            crate::db::TableSummary {
                name: "main.customers".to_string(),
            },
        ];

        let narrowed = completion_tables_for_qualifier(&tables, "orders.", &HashMap::new());

        assert_eq!(narrowed.len(), 2);
        assert!(narrowed.iter().all(|table| table.name.ends_with(".orders")));
    }

    #[test]
    fn sql_completion_schema_qualifier_excludes_other_schemas() {
        let path = temp_db_path("schema-filter");
        let attached = temp_db_path("schema-filter-attached");

        let conn = Connection::open(&path).expect("create db");
        conn.execute("CREATE TABLE id_table(id INTEGER PRIMARY KEY)", [])
            .expect("create main table");
        conn.execute(
            "ATTACH DATABASE ?1 AS other",
            [attached.to_string_lossy().into_owned()],
        )
        .expect("attach db");
        conn.execute("CREATE TABLE other.other_table(other_id INTEGER)", [])
            .expect("create attached table");
        drop(conn);

        let mut app = App::load(path.clone()).expect("load app");
        app.db
            .execute_sql(
                &format!("ATTACH DATABASE '{}' AS other", attached.display()),
                10,
            )
            .expect("attach on app connection");
        app.tables = app.db.list_tables().expect("refresh tables");
        app.sql.column_cache.clear();

        app.sql.query = "SELECT other.".to_string();
        app.sql.cursor = app.sql.query.len();
        app.sql_refresh_completion().expect("refresh completion");

        let items = app
            .sql
            .completion
            .as_ref()
            .expect("completion")
            .items
            .iter()
            .map(|item| item.insert_text.as_str())
            .collect::<Vec<_>>();

        assert!(items.contains(&"other.other_table.other_id"));
        assert!(!items.contains(&"other.id"));

        let _ = fs::remove_file(path);
        let _ = fs::remove_file(attached);
    }

    #[test]
    fn sql_completion_uses_full_table_name_for_ambiguous_bare_qualifier() {
        let path = temp_db_path("ambiguous-bare-completion");
        let attached = temp_db_path("ambiguous-bare-attached");

        let conn = Connection::open(&path).expect("create db");
        conn.execute("CREATE TABLE orders(id INTEGER PRIMARY KEY)", [])
            .expect("create main table");
        conn.execute(
            "ATTACH DATABASE ?1 AS other",
            [attached.to_string_lossy().into_owned()],
        )
        .expect("attach db");
        conn.execute("CREATE TABLE other.orders(id INTEGER PRIMARY KEY)", [])
            .expect("create attached table");
        drop(conn);

        let mut app = App::load(path.clone()).expect("load app");
        app.db
            .execute_sql(
                &format!("ATTACH DATABASE '{}' AS other", attached.display()),
                10,
            )
            .expect("attach on app connection");
        app.tables = app.db.list_tables().expect("refresh tables");
        app.sql.column_cache.clear();
        app.sql.query = "SELECT orders.".to_string();
        app.sql.cursor = app.sql.query.len();
        app.sql_refresh_completion().expect("refresh completion");

        let items = app
            .sql
            .completion
            .as_ref()
            .expect("completion")
            .items
            .iter()
            .map(|item| (item.label.as_str(), item.insert_text.as_str()))
            .collect::<Vec<_>>();

        assert!(items.contains(&("main.orders.id", "main.orders.id")));
        assert!(items.contains(&("other.orders.id", "other.orders.id")));

        let _ = fs::remove_file(path);
        let _ = fs::remove_file(attached);
    }

    #[test]
    fn sql_completion_preserves_snippets_with_keyword_labels() {
        let mut app = test_app_with_tables(
            "snippet-labels",
            &["CREATE TABLE orders(id INTEGER PRIMARY KEY)"],
        );

        let items = app
            .sql_completion_candidates("INSERT")
            .expect("completion candidates");

        assert!(
            items.iter().any(|item| {
                item.label == "INSERT INTO" && item.insert_text == "INSERT INTO  ()\nVALUES ();"
            }),
            "expected snippet completion for INSERT INTO"
        );
        assert!(
            items
                .iter()
                .any(|item| item.label == "INSERT INTO" && item.insert_text == "INSERT INTO"),
            "expected keyword completion for INSERT INTO"
        );
    }

    #[test]
    fn sql_aliases_map_aliases_to_their_tables() {
        let aliases =
            sql_aliases_before_cursor("SELECT u. FROM main.orders o JOIN main.users AS u ON u.id = o.id");

        assert_eq!(aliases.get("o"), Some(&"main.orders".to_string()));
        assert_eq!(aliases.get("u"), Some(&"main.users".to_string()));
    }

    fn temp_db_path(label: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("squid-sql-{label}-{stamp}.sqlite"))
    }

    fn test_app_with_tables(label: &str, statements: &[&str]) -> App {
        let path = temp_db_path(label);
        let conn = Connection::open(&path).expect("create db");
        for statement in statements {
            conn.execute(statement, []).expect("setup statement");
        }
        drop(conn);

        let app = App::load(path.clone()).expect("load app");
        let _ = fs::remove_file(path);
        app
    }
}
