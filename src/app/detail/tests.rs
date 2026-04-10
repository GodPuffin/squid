use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;

use crate::app::{Action, App, ContentView, DetailPane};

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
    assert!(
        detail
            .fields
            .iter()
            .any(|field| field.column_name == "customer_id" && field.foreign_target.is_some())
    );

    app.detail = None;
    app.selected_row = 1;
    app.open_detail().unwrap();
    let detail = app.detail.as_ref().unwrap();
    assert!(
        detail
            .fields
            .iter()
            .any(|field| field.column_name == "customer_id" && field.foreign_target.is_none())
    );
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
    assert!(app.detail.as_ref().unwrap().value_scroll < 100);

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
fn save_detail_changes_updates_row_and_reloads_modal() {
    let mut app = app_with_detail_data("detail-save");
    app.selected_row = 0;
    app.open_detail().unwrap();
    let field_index = app
        .detail
        .as_ref()
        .unwrap()
        .fields
        .iter()
        .position(|field| field.column_name == "notes")
        .unwrap();
    app.detail_select_field(field_index);
    app.detail_focus_value();

    app.handle_detail(Action::EditDetail).unwrap();
    for _ in "line one line two line three".chars() {
        app.handle_detail(Action::Backspace).unwrap();
    }
    for ch in "updated note".chars() {
        app.handle_detail(Action::InputChar(ch)).unwrap();
    }
    app.handle_detail(Action::SaveDetail).unwrap();

    let detail = app.detail.as_ref().unwrap();
    let field = detail
        .fields
        .iter()
        .find(|field| field.column_name == "notes")
        .unwrap();
    assert_eq!(field.original_value, "updated note");
    assert_eq!(field.draft_value, "updated note");
    assert!(
        detail
            .message
            .as_ref()
            .unwrap()
            .text
            .contains("Saved 1 field")
    );
    assert!(!app.detail_has_changes());
}

#[test]
fn discard_detail_changes_restores_original_values() {
    let mut app = app_with_detail_data("detail-discard");
    app.selected_row = 0;
    app.open_detail().unwrap();
    let field_index = app
        .detail
        .as_ref()
        .unwrap()
        .fields
        .iter()
        .position(|field| field.column_name == "notes")
        .unwrap();
    app.detail_select_field(field_index);
    app.detail_focus_value();

    app.handle_detail(Action::EditDetail).unwrap();
    for ch in " extra".chars() {
        app.handle_detail(Action::InputChar(ch)).unwrap();
    }
    assert!(app.detail_has_changes());

    app.handle_detail(Action::DiscardDetail).unwrap();

    let detail = app.detail.as_ref().unwrap();
    let field = detail
        .fields
        .iter()
        .find(|field| field.column_name == "notes")
        .unwrap();
    assert_eq!(field.original_value, "line one line two line three");
    assert_eq!(field.draft_value, "line one line two line three");
    assert!(!app.detail_has_changes());
}

#[test]
fn save_detail_changes_treats_null_literal_as_sql_null() {
    let mut app = app_with_detail_data("detail-null");
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
    app.detail_focus_value();

    app.handle_detail(Action::EditDetail).unwrap();
    for _ in "1".chars() {
        app.handle_detail(Action::Backspace).unwrap();
    }
    for ch in "NULL".chars() {
        app.handle_detail(Action::InputChar(ch)).unwrap();
    }
    app.handle_detail(Action::SaveDetail).unwrap();

    let detail = app.detail.as_ref().unwrap();
    let field = detail
        .fields
        .iter()
        .find(|field| field.column_name == "customer_id")
        .unwrap();
    assert_eq!(field.original_value, "NULL");
}

#[test]
fn invalid_integer_input_is_rejected_without_saving() {
    let mut app = app_with_detail_data("detail-int-error");
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
    app.detail_focus_value();

    app.handle_detail(Action::EditDetail).unwrap();
    for _ in "1".chars() {
        app.handle_detail(Action::Backspace).unwrap();
    }
    for ch in "abc".chars() {
        app.handle_detail(Action::InputChar(ch)).unwrap();
    }
    app.handle_detail(Action::SaveDetail).unwrap();

    let detail = app.detail.as_ref().unwrap();
    let field = detail
        .fields
        .iter()
        .find(|field| field.column_name == "customer_id")
        .unwrap();
    assert_eq!(field.original_value, "1");
    assert_eq!(field.draft_value, "abc");
    assert!(
        detail
            .message
            .as_ref()
            .unwrap()
            .text
            .contains("expects an integer")
    );
}

#[test]
fn numeric_input_is_bound_without_f64_precision_loss() {
    let path = temp_db_path("detail-numeric");
    let conn = Connection::open(&path).expect("create db");
    conn.execute_batch(
        "CREATE TABLE metrics(id INTEGER PRIMARY KEY, amount NUMERIC);
         INSERT INTO metrics(amount) VALUES (0);",
    )
    .expect("seed db");
    drop(conn);

    let mut app = App::load(path.clone()).expect("load app");
    let table_index = app
        .tables
        .iter()
        .position(|table| table.name == "main.metrics")
        .unwrap();
    app.select_table_by_index(table_index).unwrap();
    app.focus_content();
    app.open_detail().unwrap();

    let field_index = app
        .detail
        .as_ref()
        .unwrap()
        .fields
        .iter()
        .position(|field| field.column_name == "amount")
        .unwrap();
    app.detail_select_field(field_index);
    app.detail_focus_value();

    app.handle_detail(Action::EditDetail).unwrap();
    app.handle_detail(Action::Backspace).unwrap();
    for ch in "9007199254740993".chars() {
        app.handle_detail(Action::InputChar(ch)).unwrap();
    }
    app.handle_detail(Action::SaveDetail).unwrap();

    let detail = app.detail.as_ref().unwrap();
    let field = detail
        .fields
        .iter()
        .find(|field| field.column_name == "amount")
        .unwrap();
    assert_eq!(field.original_value, "9007199254740993");
    assert_eq!(field.draft_value, "9007199254740993");

    let _ = fs::remove_file(path);
}

#[test]
fn boolean_input_is_coerced_and_saved() {
    let mut app = app_with_detail_data("detail-bool");
    app.selected_row = 0;
    app.open_detail().unwrap();
    let field_index = app
        .detail
        .as_ref()
        .unwrap()
        .fields
        .iter()
        .position(|field| field.column_name == "is_priority")
        .unwrap();
    app.detail_select_field(field_index);
    app.detail_focus_value();

    app.handle_detail(Action::EditDetail).unwrap();
    for _ in "1".chars() {
        app.handle_detail(Action::Backspace).unwrap();
    }
    for ch in "false".chars() {
        app.handle_detail(Action::InputChar(ch)).unwrap();
    }
    app.handle_detail(Action::SaveDetail).unwrap();

    let detail = app.detail.as_ref().unwrap();
    let field = detail
        .fields
        .iter()
        .find(|field| field.column_name == "is_priority")
        .unwrap();
    assert_eq!(field.original_value, "0");
    assert_eq!(field.draft_value, "0");
}

#[test]
fn without_rowid_tables_open_as_read_only_details() {
    let path = temp_db_path("detail-read-only");
    let conn = Connection::open(&path).expect("create db");
    conn.execute_batch(
        "CREATE TABLE codes(
            id TEXT PRIMARY KEY,
            label TEXT
        ) WITHOUT ROWID;
        INSERT INTO codes(id, label) VALUES ('A', 'alpha');",
    )
    .expect("seed db");
    drop(conn);

    let mut app = App::load(path.clone()).expect("load app");
    app.focus_content();
    app.open_detail().unwrap();

    let detail = app.detail.as_ref().unwrap();
    assert_eq!(detail.rowid, None);
    assert!(!app.detail_is_row_writable());
    assert!(
        detail
            .message
            .as_ref()
            .unwrap()
            .text
            .contains("Read-only row")
    );

    let _ = fs::remove_file(path);
}

#[test]
fn read_only_databases_open_details_without_edit_affordances() {
    let path = temp_db_path("detail-readonly-db");
    let conn = Connection::open(&path).expect("create db");
    conn.execute("CREATE TABLE items(id INTEGER PRIMARY KEY, label TEXT)", [])
        .expect("create table");
    conn.execute("INSERT INTO items(label) VALUES ('alpha')", [])
        .expect("seed db");
    drop(conn);

    let uri = PathBuf::from(format!("file:{}?mode=ro", path.display()));
    let mut app = App::load(uri).expect("load app");
    app.focus_content();
    app.open_detail().unwrap();

    let detail = app.detail.as_ref().unwrap();
    assert_eq!(detail.rowid, Some(1));
    assert!(!app.detail_database_is_writable());
    assert!(!app.detail_is_row_writable());
    assert!(
        detail
            .message
            .as_ref()
            .unwrap()
            .text
            .contains("Read-only database")
    );

    let _ = fs::remove_file(path);
}

#[test]
fn read_only_attached_tables_open_details_without_edit_affordances() {
    let main_path = temp_db_path("detail-attached-main");
    let attached_path = temp_db_path("detail-attached-readonly");

    let conn = Connection::open(&main_path).expect("create main db");
    conn.execute(
        "CREATE TABLE local_items(id INTEGER PRIMARY KEY, label TEXT)",
        [],
    )
    .expect("create main table");
    conn.execute("INSERT INTO local_items(label) VALUES ('local')", [])
        .expect("seed main table");
    drop(conn);

    let conn = Connection::open(&attached_path).expect("create attached db");
    conn.execute("CREATE TABLE items(id INTEGER PRIMARY KEY, label TEXT)", [])
        .expect("create attached table");
    conn.execute("INSERT INTO items(label) VALUES ('remote')", [])
        .expect("seed attached table");
    drop(conn);

    let mut app = App::load(main_path.clone()).expect("load app");
    app.db
        .as_ref()
        .unwrap()
        .execute_sql(
            &format!(
                "ATTACH DATABASE 'file:{}?mode=ro' AS other",
                attached_path.display()
            ),
            10,
        )
        .expect("attach readonly schema");
    app.refresh_loaded_db_state().expect("refresh tables");
    app.select_table_by_name("other.items").unwrap();
    app.focus_content();
    app.open_detail().unwrap();

    let detail = app.detail.as_ref().unwrap();
    assert_eq!(detail.rowid, Some(1));
    assert!(!app.detail_database_is_writable());
    assert!(!app.detail_is_row_writable());
    assert!(
        detail
            .message
            .as_ref()
            .unwrap()
            .text
            .contains("Read-only database")
    );

    let _ = fs::remove_file(main_path);
    let _ = fs::remove_file(attached_path);
}

#[test]
fn detail_focus_value_keeps_existing_edit_mode() {
    let mut app = app_with_detail_data("detail-focus-edit");
    app.selected_row = 0;
    app.open_detail().unwrap();

    let field_index = app
        .detail
        .as_ref()
        .unwrap()
        .fields
        .iter()
        .position(|field| field.column_name == "notes")
        .unwrap();
    app.detail_select_field(field_index);
    app.detail_focus_value();
    app.handle_detail(Action::EditDetail).unwrap();

    assert!(app.detail_is_editing());
    app.detail_focus_value();
    assert!(app.detail_is_editing());
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
             notes TEXT,
             is_priority BOOLEAN NOT NULL DEFAULT 0
         );
         INSERT INTO customers(name) VALUES ('alice'), ('bravo');
         INSERT INTO orders(customer_id, notes, is_priority) VALUES
             (1, 'line one line two line three', 1),
             (NULL, 'orphan row', 0);",
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
