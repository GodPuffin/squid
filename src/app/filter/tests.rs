use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;

use crate::app::{Action, App, FilterPane, FilterRule};
use crate::db::FilterMode;

#[test]
fn open_filter_modal_initializes_and_uses_column_type_modes() {
    let mut app = app_with_filters("filter-init");

    app.open_filter_modal();
    assert_eq!(app.filter_modal_pane(), Some(FilterPane::Columns));
    assert_eq!(
        app.filter_modal_mode_lines(),
        vec!["contains", "equals", "starts with"]
    );

    app.filter_modal_select_column(1);
    assert_eq!(
        app.filter_modal_mode_lines(),
        vec!["equals", "greater than", "less than"]
    );

    app.filter_modal_select_column(2);
    assert_eq!(app.filter_modal_mode_lines(), vec!["is true", "is false"]);
}

#[test]
fn confirm_replaces_existing_rule_with_trimmed_value() {
    let mut app = app_with_filters("filter-replace");

    app.open_filter_modal();
    app.filter_modal_focus_draft();
    app.filter_modal.as_mut().unwrap().input = "  alice  ".to_string();
    app.handle_filter_modal(Action::Confirm).unwrap();

    assert_eq!(app.current_filter_rules().len(), 1);
    assert_eq!(app.current_filter_rules()[0].value, "alice");

    app.filter_modal_focus_draft();
    app.filter_modal.as_mut().unwrap().input = "bob".to_string();
    app.handle_filter_modal(Action::Confirm).unwrap();

    assert_eq!(
        app.current_filter_rules()
            .into_iter()
            .map(|rule| (rule.column_index, rule.mode, rule.value))
            .collect::<Vec<_>>(),
        vec![(0, FilterMode::Contains, "bob".to_string())]
    );
}

#[test]
fn boolean_filter_applies_without_input_and_blank_text_is_ignored() {
    let mut app = app_with_filters("filter-boolean");

    app.open_filter_modal();
    app.filter_modal_select_column(0);
    app.filter_modal_focus_draft();
    app.filter_modal.as_mut().unwrap().input = "   ".to_string();
    app.handle_filter_modal(Action::Confirm).unwrap();
    assert!(app.current_filter_rules().is_empty());

    app.filter_modal_select_column(2);
    app.handle_filter_modal(Action::Confirm).unwrap();

    assert_eq!(
        app.current_filter_rules()
            .into_iter()
            .map(|rule| (rule.column_index, rule.mode, rule.value))
            .collect::<Vec<_>>(),
        vec![(2, FilterMode::IsTrue, String::new())]
    );
}

#[test]
fn delete_and_clear_remove_rules_and_sync_draft() {
    let mut app = app_with_filters("filter-delete");
    seed_filter_rules(&mut app);

    app.open_filter_modal();
    app.filter_modal_select_active(1);
    app.handle_filter_modal(Action::Delete).unwrap();

    assert_eq!(app.current_filter_rules().len(), 1);
    assert_eq!(app.modal_filter_column_name(), "name");

    app.handle_filter_modal(Action::Clear).unwrap();
    assert!(app.current_filter_rules().is_empty());
    assert_eq!(app.modal_filter_active_lines(), vec!["No active filters"]);
}

#[test]
fn draft_space_only_inserts_for_input_modes() {
    let mut app = app_with_filters("filter-space");

    app.open_filter_modal();
    app.filter_modal_focus_draft();
    app.handle_filter_modal(Action::ToggleItem).unwrap();
    assert_eq!(app.modal_filter_input(), " ");

    app.filter_modal_select_column(2);
    app.filter_modal_focus_draft();
    app.handle_filter_modal(Action::ToggleItem).unwrap();
    assert_eq!(app.active_filter_mode(), Some(FilterMode::IsFalse));
    assert_eq!(app.modal_filter_input(), "");
}

#[test]
fn space_toggles_column_visibility_from_filter_modal_columns_pane() {
    let mut app = app_with_filters("filter-column-toggle");

    app.open_filter_modal();
    app.handle_filter_modal(Action::ToggleItem).unwrap();
    assert_eq!(app.visible_column_flags(), vec![false, true, true]);

    app.filter_modal_select_column(1);
    app.handle_filter_modal(Action::ToggleItem).unwrap();
    assert_eq!(app.visible_column_flags(), vec![false, false, true]);

    app.filter_modal_select_column(2);
    app.handle_filter_modal(Action::ToggleItem).unwrap();
    assert_eq!(app.visible_column_flags(), vec![false, false, true]);
}

fn seed_filter_rules(app: &mut App) {
    let selected = app.selected_table_name().unwrap().to_string();
    app.configs.insert(
        selected,
        crate::app::TableConfig {
            visible_columns: vec![true, true, true],
            sort_clauses: Vec::new(),
            filter_rules: vec![
                FilterRule {
                    column_index: 0,
                    mode: FilterMode::Contains,
                    value: "alpha".to_string(),
                },
                FilterRule {
                    column_index: 2,
                    mode: FilterMode::IsTrue,
                    value: String::new(),
                },
            ],
        },
    );
}

fn app_with_filters(label: &str) -> App {
    let path = temp_db_path(label);
    let conn = Connection::open(&path).expect("create db");
    conn.execute_batch(
        "CREATE TABLE demo(
            name TEXT,
            age INTEGER,
            active BOOLEAN
        );
        INSERT INTO demo(name, age, active) VALUES
            ('alice', 30, 1),
            ('bob', 40, 0);",
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
    std::env::temp_dir().join(format!("squid-filter-{label}-{stamp}.sqlite"))
}
