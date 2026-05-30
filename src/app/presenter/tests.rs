use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;

use crate::app::{
    App, AppMode, FilterPane, FilterRule, ModalPane, ModalState, RecentItem, SearchScope,
    SearchState, SortRule, TableConfig,
};
use crate::db::FilterMode;

#[test]
fn content_title_includes_hidden_filter_and_sort_summaries() {
    let mut app = app_with_presenter_data("presenter-title");
    let table_name = app.selected_table_name().unwrap().to_string();
    app.configs.insert(
        table_name,
        TableConfig {
            visible_columns: vec![true, false, true],
            sort_clauses: vec![SortRule {
                column_index: 2,
                descending: true,
            }],
            filter_rules: vec![FilterRule {
                column_index: 0,
                mode: FilterMode::Contains,
                value: "alphabet soup".to_string(),
            }],
        },
    );

    let title = app.content_title();
    assert!(title.contains("+1 hidden"));
    assert!(title.contains("sort: active desc"));
    assert!(title.contains("filter: name~alphabet sou…"));
}

#[test]
fn footer_hint_is_compact_and_offers_help() {
    let mut app = app_with_presenter_data("presenter-footer");
    let hint = app.footer_hint();
    assert!(hint.contains("Enter"));
    assert!(hint.contains("? help"));
    assert!(hint.len() < 60);

    app.modal = Some(ModalState {
        pane: ModalPane::Columns,
        column_index: 0,
        sort_column_index: 0,
        sort_active_index: 0,
        pending_desc: false,
    });
    assert!(app.footer_hint().contains("Enter sort"));

    app.modal = None;
    app.open_filter_modal();
    assert_eq!(app.filter_modal_pane(), Some(FilterPane::Columns));
    assert!(app.footer_hint().contains("Enter apply"));
}

#[test]
fn help_entries_cover_browse_actions() {
    let app = app_with_presenter_data("presenter-help-entries");
    let keys: Vec<_> = app
        .help_entries()
        .into_iter()
        .map(|entry| entry.key)
        .collect();
    assert!(keys.iter().any(|key| key.contains("Enter")));
    assert!(keys.iter().any(|key| key.contains('f')));
    assert!(keys.iter().any(|key| key.contains('m')));
    assert!(keys.iter().any(|key| key.contains(',')));
}

#[test]
fn home_footer_includes_settings_and_confirm_remove() {
    let mut app = App::load(None).unwrap();
    app.mode = AppMode::Home;
    app.path = None;
    app.db = None;
    app.recent_items.clear();
    assert!(app.is_home());
    assert!(app.footer_hint().contains(", settings"));

    app.pending_recent_removal = Some(std::env::temp_dir().join("missing.db"));
    assert!(app.footer_hint().contains("Del confirm"));
}

#[test]
fn help_unavailable_while_searching() {
    let mut app = app_with_presenter_data("presenter-search-help");
    app.open_search(SearchScope::CurrentTable).unwrap();
    assert!(!app.help_available());
    assert!(!app.footer_hint().contains("? help"));
}

#[test]
fn detail_help_lists_delete_with_pending_changes() {
    let mut app = app_with_presenter_data("presenter-detail-delete-help");
    app.focus = crate::app::PaneFocus::Content;
    app.content_view = crate::app::ContentView::Rows;
    app.open_detail().unwrap();
    if let Some(detail) = app.detail.as_mut() {
        if let Some(field) = detail.fields.first_mut() {
            field.draft_value.push('!');
        }
    }

    let keys: Vec<_> = app
        .help_entries()
        .into_iter()
        .map(|entry| entry.key)
        .collect();
    assert!(keys.iter().any(|key| key == "d"));
}

#[test]
fn detail_footer_hint_stays_compact() {
    let mut app = app_with_presenter_data("presenter-detail-footer");
    app.focus = crate::app::PaneFocus::Content;
    app.content_view = crate::app::ContentView::Rows;
    app.open_detail().unwrap();
    assert!(app.detail.is_some());
    let hint = app.footer_hint();
    assert!(hint.contains("? help"));
    assert!(hint.len() < 60);
}

#[test]
fn footer_hint_matches_current_table_search_mode() {
    let mut app = app_with_presenter_data("presenter-search-footer");
    app.open_search(SearchScope::CurrentTable).unwrap();
    assert!(app.footer_hint().contains("Type"));

    app.preview.total_rows = 2_001;
    app.search = Some(SearchState {
        scope: SearchScope::CurrentTable,
        query: "needle".to_string(),
        results: Vec::new(),
        selected_result: 0,
        result_offset: 0,
        horizontal_offset: 0,
        result_limit: 10,
        submitted: true,
        loading: false,
    });
    assert!(app.footer_hint().contains("Type"));

    app.search.as_mut().unwrap().loading = true;
    assert!(app.footer_hint().contains("Searching table"));
    assert!(!app.footer_hint().contains("Esc close"));
}

#[test]
fn modal_and_filter_lines_render_empty_states() {
    let mut app = App::load(None).unwrap();
    app.recent_items.clear();
    assert_eq!(app.home_recent_lines(), vec!["No recent files"]);

    app = app_with_presenter_data("presenter-lines");
    assert_eq!(app.modal_sort_active_lines(), vec!["No active sort"]);
    assert_eq!(app.modal_filter_active_lines(), vec!["No active filters"]);
}

#[test]
fn home_recent_lines_lead_with_filename_for_long_paths() {
    let mut app = App::load(None).unwrap();
    let path = std::env::temp_dir()
        .join("squid-presenter")
        .join("nested")
        .join("deeper")
        .join("sakila.db");
    app.recent_items = vec![
        RecentItem {
            path: path.clone(),
            available: true,
        },
        RecentItem {
            path,
            available: false,
        },
    ];

    let lines = app.home_recent_lines();
    assert!(lines[0].starts_with("sakila.db"));
    assert!(!lines[0].contains("[missing]"));
    assert!(lines[1].starts_with("sakila.db"));
    assert!(lines[1].ends_with("[missing]"));
}

#[test]
fn create_sql_and_empty_type_are_formatted_for_schema_lines() {
    let mut app = app_with_presenter_data("presenter-schema");
    app.details.as_mut().unwrap().columns[1].data_type.clear();

    let lines = app.schema_lines();
    assert!(lines.iter().any(|line| line.contains("UNKNOWN")));
    assert!(lines.iter().any(|line| line == "Create SQL"));
}

fn app_with_presenter_data(label: &str) -> App {
    let path = temp_db_path(label);
    let conn = Connection::open(&path).expect("create db");
    conn.execute_batch(
        "CREATE TABLE demo(
            name TEXT,
            misc TEXT,
            active BOOLEAN
        );
        INSERT INTO demo(name, misc, active) VALUES ('alice', 'notes', 1);",
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
    std::env::temp_dir().join(format!("squid-presenter-{label}-{stamp}.sqlite"))
}

#[test]
fn truncate_cell_preview_respects_limit() {
    use super::truncate_cell_preview;

    assert_eq!(truncate_cell_preview("hello", 0), "hello");
    assert_eq!(truncate_cell_preview("hello", 10), "hello");
    assert_eq!(truncate_cell_preview("hello world", 5), "hello…");
}
