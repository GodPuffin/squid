use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;

use crate::app::{Action, App, ModalPane, SortRule, TableConfig};

#[test]
fn open_modal_initializes_pane_and_indices() {
    let mut app = app_with_modal("modal-init");

    app.open_config_modal();

    assert_eq!(app.modal_pane(), Some(ModalPane::Columns));
    assert_eq!(app.modal_selected_indices(), (Some(0), Some(0), None));
}

#[test]
fn toggle_column_keeps_one_visible_column_minimum() {
    let mut app = app_with_modal("modal-toggle");
    let table_name = app.selected_table_name().unwrap().to_string();
    app.configs.insert(
        table_name,
        TableConfig {
            visible_columns: vec![true, false],
            sort_clauses: Vec::new(),
            filter_rules: Vec::new(),
        },
    );

    app.open_config_modal();
    app.modal_click_columns(0).unwrap();

    assert_eq!(app.visible_column_flags(), vec![true, false]);
}

#[test]
fn confirm_sort_adds_and_updates_existing_rule() {
    let mut app = app_with_modal("modal-sort");

    app.open_config_modal();
    app.modal_click_sort_candidate(1, false).unwrap();
    assert_eq!(
        app.current_sort_rules()
            .into_iter()
            .map(|rule| (rule.column_index, rule.descending))
            .collect::<Vec<_>>(),
        vec![(1, false)]
    );

    app.modal_click_sort_candidate(1, true).unwrap();
    assert_eq!(
        app.current_sort_rules()
            .into_iter()
            .map(|rule| (rule.column_index, rule.descending))
            .collect::<Vec<_>>(),
        vec![(1, true)]
    );
}

#[test]
fn toggling_active_sort_flips_direction_and_delete_clamps_selection() {
    let mut app = app_with_modal("modal-active");
    let table_name = app.selected_table_name().unwrap().to_string();
    app.configs.insert(
        table_name,
        TableConfig {
            visible_columns: vec![true, true],
            sort_clauses: vec![
                SortRule {
                    column_index: 0,
                    descending: false,
                },
                SortRule {
                    column_index: 1,
                    descending: true,
                },
            ],
            filter_rules: Vec::new(),
        },
    );

    app.open_config_modal();
    app.modal_select_sort_rule(1);
    app.handle_modal(Action::ToggleItem).unwrap();
    assert_eq!(
        app.current_sort_rules()
            .into_iter()
            .map(|rule| (rule.column_index, rule.descending))
            .collect::<Vec<_>>(),
        vec![(0, false), (1, false)]
    );

    app.modal_remove_sort_rule(1).unwrap();
    assert_eq!(app.current_sort_rules().len(), 1);
    assert_eq!(app.modal_selected_indices(), (Some(0), Some(0), Some(0)));
}

#[test]
fn clear_sorts_resets_active_index() {
    let mut app = app_with_modal("modal-clear");
    let table_name = app.selected_table_name().unwrap().to_string();
    app.configs.insert(
        table_name,
        TableConfig {
            visible_columns: vec![true, true],
            sort_clauses: vec![SortRule {
                column_index: 0,
                descending: false,
            }],
            filter_rules: Vec::new(),
        },
    );

    app.open_config_modal();
    app.modal_select_sort_rule(0);
    app.handle_modal(Action::Clear).unwrap();

    assert!(app.current_sort_rules().is_empty());
    assert_eq!(app.modal_selected_indices(), (Some(0), Some(0), None));
}

fn app_with_modal(label: &str) -> App {
    let path = temp_db_path(label);
    let conn = Connection::open(&path).expect("create db");
    conn.execute_batch(
        "CREATE TABLE demo(
            name TEXT,
            age INTEGER
        );
        INSERT INTO demo(name, age) VALUES ('alice', 30), ('bob', 40);",
    )
    .expect("seed db");
    drop(conn);

    let app = App::load(path.clone()).expect("load app");
    let _ = fs::remove_file(path);
    app
}

fn temp_db_path(label: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("squid-modal-{label}-{stamp}.sqlite"))
}
