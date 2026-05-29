use anyhow::Result;

use crate::app::home::AppStorage;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppSettings {
    pub mouse_enabled: bool,
    pub restore_session_on_open: bool,
    pub auto_open_last_database: bool,
    pub recent_limit: usize,
    pub sql_result_row_limit: usize,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            mouse_enabled: true,
            restore_session_on_open: true,
            auto_open_last_database: true,
            recent_limit: 10,
            sql_result_row_limit: 200,
        }
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
