use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;

use crate::app::{App, ContentView, DetailPane};

#[test]
fn open_detail_is_noop_outside_rows_or_without_rows() {
    let mut app = app_with_detail_data("detail-noop");
    app.content_view = ContentView::Schema;
    app.open_detail().unwrap();
    assert!(app.detail.is_none());

    app.content_view = ContentView::Rows;
    app.preview.total_rows = 0;
    app.open_detail().unwrap();
    assert!(app.detail.is_none());
}

#[test]
fn open_detail_builds_foreign_targets_and_skips_null_links() {
    let mut app = app_with_detail_data("detail-links");
    app.selected_row = 0;
    app.open_detail().unwrap();

    let detail = app.detail.as_ref().unwrap();
    assert!(detail
        .fields
        .iter()
        .any(|field| field.column_name == "customer_id" && field.foreign_target.is_some()));

    app.detail = None;
    app.selected_row = 1;
    app.open_detail().unwrap();
    let detail = app.detail.as_ref().unwrap();
    assert!(detail
        .fields
        .iter()
        .any(|field| field.column_name == "customer_id" && field.foreign_target.is_none()));
}

#[test]
fn detail_selection_and_scroll_are_clamped() {
    let mut app = app_with_detail_data("detail-scroll");
    app.selected_row = 0;
    app.open_detail().unwrap();

    {
        let detail = app.detail.as_mut().unwrap();
        detail.value_view_width = 4;
        detail.value_view_height = 2;
        detail.selected_field = 2;
        detail.value_scroll = 100;
    }
    app.clamp_detail_scroll();
    assert!(app.detail.as_ref().unwrap().value_scroll <= 5);

    app.detail.as_mut().unwrap().value_scroll = 3;
    app.detail_select_field(0);
    let detail = app.detail.as_ref().unwrap();
    assert_eq!(detail.pane, DetailPane::Fields);
    assert_eq!(detail.value_scroll, 0);
}

#[test]
fn follow_detail_link_selects_foreign_row_and_closes_detail() {
    let mut app = app_with_detail_data("detail-follow");
    app.selected_row = 0;
    app.open_detail().unwrap();
    let field_index = app
        .detail
        .as_ref()
        .unwrap()
        .fields
        .iter()
        .position(|field| field.column_name == "customer_id")
        .unwrap();
    app.detail_select_field(field_index);

    app.follow_detail_link().unwrap();

    assert!(app.detail.is_none());
    assert_eq!(app.selected_table_name(), Some("main.customers"));
    assert_eq!(app.selected_row, 0);
}

#[test]
fn wrapped_line_count_handles_empty_and_wrapped_values() {
    assert_eq!(super::wrapped_line_count("", 4), 1);
    assert_eq!(super::wrapped_line_count("abcdef", 4), 2);
    assert_eq!(super::wrapped_line_count("ab\ncdefg", 3), 3);
}

fn app_with_detail_data(label: &str) -> App {
    let path = temp_db_path(label);
    let conn = Connection::open(&path).expect("create db");
    conn.execute_batch(
        "PRAGMA foreign_keys = ON;
         CREATE TABLE customers(id INTEGER PRIMARY KEY, name TEXT);
         CREATE TABLE orders(
             id INTEGER PRIMARY KEY,
             customer_id INTEGER REFERENCES customers(id),
             notes TEXT
         );
         INSERT INTO customers(name) VALUES ('alice'), ('bravo');
         INSERT INTO orders(customer_id, notes) VALUES
             (1, 'line one line two line three'),
             (NULL, 'orphan row');",
    )
    .expect("seed db");
    drop(conn);

    let mut app = App::load(path.clone()).expect("load app");
    let orders_index = app
        .tables
        .iter()
        .position(|table| table.name == "main.orders")
        .unwrap();
    app.select_table_by_index(orders_index).unwrap();
    app.focus_content();
    let _ = fs::remove_file(path);
    app
}

fn temp_db_path(label: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("squid-detail-{label}-{stamp}.sqlite"))
}
