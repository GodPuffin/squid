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
        .as_ref()
        .expect("db loaded")
        .execute_sql(
            &format!("ATTACH DATABASE '{}' AS other", attached.display()),
            10,
        )
        .expect("attach on app connection");
    app.tables = app
        .db
        .as_ref()
        .expect("db loaded")
        .list_tables()
        .expect("refresh tables");
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
        .as_ref()
        .expect("db loaded")
        .execute_sql(
            &format!("ATTACH DATABASE '{}' AS other", attached.display()),
            10,
        )
        .expect("attach on app connection");
    app.tables = app
        .db
        .as_ref()
        .expect("db loaded")
        .list_tables()
        .expect("refresh tables");
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
    let aliases = sql_aliases_before_cursor(
        "SELECT u. FROM main.orders o JOIN main.users AS u ON u.id = o.id",
    );

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
