use std::path::PathBuf;

use anyhow::Result;

use crate::db::RowPreview;

use super::super::{
    App, AppMode, ContentView, PaneFocus, RecentStore, SqlPane, SqlResultState, SqlState,
    home::{AppStorage, recent_path_is_available},
};

impl App {
    pub fn load(path: impl Into<Option<PathBuf>>) -> Result<Self> {
        let path = match path.into() {
            Some(path) => Some(path),
            None => AppStorage::last_opened_path().unwrap_or(None),
        };
        let (recent_items, status_message) = match RecentStore::load() {
            Ok(items) => (items, None),
            Err(error) => (Vec::new(), Some(format!("Could not load recents: {error}"))),
        };

        let mut app = Self {
            mode: AppMode::Home,
            path: None,
            db: None,
            tables: Vec::new(),
            selected_table: 0,
            focus: PaneFocus::Tables,
            content_view: ContentView::Rows,
            row_offset: 0,
            row_limit: super::super::DEFAULT_ROW_LIMIT,
            selected_row: 0,
            schema_offset: 0,
            schema_page_lines: super::super::DEFAULT_SCHEMA_PAGE_LINES,
            preview: RowPreview::empty(),
            details: None,
            detail: None,
            filter_modal: None,
            modal: None,
            search: None,
            search_results_view_width: 0,
            recent_items,
            selected_recent: 0,
            status_message,
            sql: default_sql_state(),
            configs: std::collections::HashMap::new(),
        };

        if let Some(path) = path.filter(|path| recent_path_is_available(path)) {
            if let Err(error) = app.open_database(&path) {
                app.status_message = Some(format!("Could not restore {}: {error}", path.display()));
                app.refresh_home_selection();
            }
        } else {
            app.refresh_home_selection();
        }

        Ok(app)
    }
}

fn default_sql_state() -> SqlState {
    SqlState {
        query: String::new(),
        cursor: 0,
        editor_scroll: 0,
        editor_col_offset: 0,
        editor_height: 8,
        editor_width: 40,
        focus: SqlPane::Editor,
        history: Vec::new(),
        history_offset: 0,
        history_height: 8,
        selected_history: 0,
        result: SqlResultState::Empty,
        result_scroll: 0,
        result_height: 8,
        completion: None,
        status: "SQL mode ready".to_string(),
        column_cache: std::collections::HashMap::new(),
        completion_cache_query: String::new(),
        completion_candidates_cache: std::collections::HashMap::new(),
    }
}
