use anyhow::Result;

use crate::app::ContentView;
use crate::app::home::AppStorage;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultBrowseView {
    Rows,
    Schema,
}

impl DefaultBrowseView {
    pub fn label(self) -> &'static str {
        match self {
            DefaultBrowseView::Rows => "rows",
            DefaultBrowseView::Schema => "schema",
        }
    }

    pub fn to_content_view(self) -> ContentView {
        match self {
            DefaultBrowseView::Rows => ContentView::Rows,
            DefaultBrowseView::Schema => ContentView::Schema,
        }
    }

    pub fn toggle(self) -> Self {
        match self {
            DefaultBrowseView::Rows => DefaultBrowseView::Schema,
            DefaultBrowseView::Schema => DefaultBrowseView::Rows,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppSettings {
    pub restore_session_on_open: bool,
    pub auto_open_last_database: bool,
    pub recent_limit: usize,
    pub sql_result_row_limit: usize,
    pub confirm_before_remove_recent: bool,
    pub default_browse_view: DefaultBrowseView,
    pub sql_history_size: usize,
    pub live_table_search: bool,
    pub show_row_numbers: bool,
    pub cell_preview_max_chars: usize,
    pub double_click_interval_ms: u64,
    pub restore_cursor_on_startup: bool,
    pub clear_session_on_quit: bool,
    pub color_scheme: crate::theme::ColorScheme,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            restore_session_on_open: true,
            auto_open_last_database: true,
            recent_limit: 10,
            sql_result_row_limit: 200,
            confirm_before_remove_recent: true,
            default_browse_view: DefaultBrowseView::Rows,
            sql_history_size: 50,
            live_table_search: true,
            show_row_numbers: true,
            cell_preview_max_chars: 200,
            double_click_interval_ms: 500,
            restore_cursor_on_startup: true,
            clear_session_on_quit: false,
            color_scheme: crate::theme::ColorScheme::Dark,
        }
    }
}

impl AppSettings {
    pub fn default_content_view(&self) -> ContentView {
        self.default_browse_view.to_content_view()
    }

    pub fn double_click_interval(&self) -> std::time::Duration {
        std::time::Duration::from_millis(self.double_click_interval_ms)
    }
}

pub(crate) struct SettingsStorage;

impl SettingsStorage {
    pub fn load() -> Result<AppSettings> {
        AppStorage::load_settings()
    }

    pub fn save(settings: &AppSettings) -> Result<()> {
        AppStorage::save_settings(settings)
    }
}
