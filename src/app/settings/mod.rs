mod storage;

use anyhow::Result;

use storage::SettingsStorage;

pub use storage::{AppSettings, DefaultBrowseView};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettingId {
    ColorScheme,
    DoubleClickIntervalMs,
    ConfirmBeforeRemoveRecent,
    ConfirmBeforeDeleteRow,
    AutoOpenLastDatabase,
    RestoreSessionOnOpen,
    RestoreCursorOnStartup,
    ClearSessionOnQuit,
    DefaultBrowseView,
    RecentLimit,
    ShowRowNumbers,
    CellPreviewMaxChars,
    LiveTableSearch,
    SqlHistorySize,
    SqlResultRowLimit,
}

impl SettingId {
    pub const ALL: [SettingId; 15] = [
        SettingId::ColorScheme,
        SettingId::DoubleClickIntervalMs,
        SettingId::ConfirmBeforeRemoveRecent,
        SettingId::ConfirmBeforeDeleteRow,
        SettingId::AutoOpenLastDatabase,
        SettingId::RestoreSessionOnOpen,
        SettingId::RestoreCursorOnStartup,
        SettingId::ClearSessionOnQuit,
        SettingId::DefaultBrowseView,
        SettingId::RecentLimit,
        SettingId::ShowRowNumbers,
        SettingId::CellPreviewMaxChars,
        SettingId::LiveTableSearch,
        SettingId::SqlHistorySize,
        SettingId::SqlResultRowLimit,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SettingId::ColorScheme => "Color scheme",
            SettingId::DoubleClickIntervalMs => "Double-click interval (ms)",
            SettingId::ConfirmBeforeRemoveRecent => "Confirm before removing recent",
            SettingId::ConfirmBeforeDeleteRow => "Confirm before deleting row",
            SettingId::AutoOpenLastDatabase => "Open last database on startup",
            SettingId::RestoreSessionOnOpen => "Restore session when reopening a database",
            SettingId::RestoreCursorOnStartup => "Restore table and row on startup",
            SettingId::ClearSessionOnQuit => "Clear session data on quit",
            SettingId::DefaultBrowseView => "Default browse view",
            SettingId::RecentLimit => "Recent files limit",
            SettingId::ShowRowNumbers => "Show row numbers in preview",
            SettingId::CellPreviewMaxChars => "Cell preview max characters",
            SettingId::LiveTableSearch => "Live table search",
            SettingId::SqlHistorySize => "SQL history size",
            SettingId::SqlResultRowLimit => "SQL result row limit",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            SettingId::ColorScheme => "Palette used across the entire interface",
            SettingId::DoubleClickIntervalMs => {
                "Maximum delay between clicks to count as a double-click"
            }
            SettingId::ConfirmBeforeRemoveRecent => {
                "Require a second Delete press before removing a recent database"
            }
            SettingId::ConfirmBeforeDeleteRow => {
                "Require a second d press before deleting a row from browse or detail"
            }
            SettingId::AutoOpenLastDatabase => {
                "When no database path is passed on the command line, open the most recent file"
            }
            SettingId::RestoreSessionOnOpen => {
                "Restore filters, sorts, and SQL editor when opening a file"
            }
            SettingId::RestoreCursorOnStartup => {
                "Reopen the last table and row when restoring a database session"
            }
            SettingId::ClearSessionOnQuit => {
                "Forget per-database UI state when quitting instead of saving it"
            }
            SettingId::DefaultBrowseView => {
                "Whether browse mode starts on row data or schema when no session is restored"
            }
            SettingId::RecentLimit => "How many recent database paths are kept",
            SettingId::ShowRowNumbers => "Show the # column in the row preview table",
            SettingId::CellPreviewMaxChars => {
                "Truncate long cell values in the row preview (0 = no limit)"
            }
            SettingId::LiveTableSearch => {
                "Filter the current table as you type when the table is small enough"
            }
            SettingId::SqlHistorySize => "Maximum SQL queries kept in history per database",
            SettingId::SqlResultRowLimit => "Maximum rows returned for SELECT queries in SQL mode",
        }
    }

    pub fn display_value(self, settings: &AppSettings) -> String {
        match self {
            SettingId::ColorScheme => settings.color_scheme.label().to_string(),
            SettingId::DoubleClickIntervalMs => settings.double_click_interval_ms.to_string(),
            SettingId::ConfirmBeforeRemoveRecent => {
                bool_label(settings.confirm_before_remove_recent)
            }
            SettingId::ConfirmBeforeDeleteRow => {
                bool_label(settings.confirm_before_delete_row)
            }
            SettingId::AutoOpenLastDatabase => bool_label(settings.auto_open_last_database),
            SettingId::RestoreSessionOnOpen => bool_label(settings.restore_session_on_open),
            SettingId::RestoreCursorOnStartup => bool_label(settings.restore_cursor_on_startup),
            SettingId::ClearSessionOnQuit => bool_label(settings.clear_session_on_quit),
            SettingId::DefaultBrowseView => settings.default_browse_view.label().to_string(),
            SettingId::RecentLimit => settings.recent_limit.to_string(),
            SettingId::ShowRowNumbers => bool_label(settings.show_row_numbers),
            SettingId::CellPreviewMaxChars => {
                if settings.cell_preview_max_chars == 0 {
                    "off".to_string()
                } else {
                    settings.cell_preview_max_chars.to_string()
                }
            }
            SettingId::LiveTableSearch => bool_label(settings.live_table_search),
            SettingId::SqlHistorySize => settings.sql_history_size.to_string(),
            SettingId::SqlResultRowLimit => settings.sql_result_row_limit.to_string(),
        }
    }

    pub fn adjust(self, settings: &mut AppSettings) {
        match self {
            SettingId::ColorScheme => {
                settings.color_scheme = settings.color_scheme.cycle();
            }
            SettingId::DoubleClickIntervalMs => {
                settings.double_click_interval_ms = cycle_u64(
                    settings.double_click_interval_ms,
                    &[200, 300, 400, 500, 750, 1000],
                );
            }
            SettingId::ConfirmBeforeRemoveRecent => {
                settings.confirm_before_remove_recent = !settings.confirm_before_remove_recent;
            }
            SettingId::ConfirmBeforeDeleteRow => {
                settings.confirm_before_delete_row = !settings.confirm_before_delete_row;
            }
            SettingId::AutoOpenLastDatabase => {
                settings.auto_open_last_database = !settings.auto_open_last_database;
            }
            SettingId::RestoreSessionOnOpen => {
                settings.restore_session_on_open = !settings.restore_session_on_open;
            }
            SettingId::RestoreCursorOnStartup => {
                settings.restore_cursor_on_startup = !settings.restore_cursor_on_startup;
            }
            SettingId::ClearSessionOnQuit => {
                settings.clear_session_on_quit = !settings.clear_session_on_quit;
            }
            SettingId::DefaultBrowseView => {
                settings.default_browse_view = settings.default_browse_view.toggle();
            }
            SettingId::RecentLimit => {
                settings.recent_limit =
                    cycle_usize(settings.recent_limit, &[5, 10, 15, 20, 25, 30, 40, 50]);
            }
            SettingId::ShowRowNumbers => settings.show_row_numbers = !settings.show_row_numbers,
            SettingId::CellPreviewMaxChars => {
                settings.cell_preview_max_chars = cycle_usize(
                    settings.cell_preview_max_chars,
                    &[0, 40, 80, 120, 200, 400, 800],
                );
            }
            SettingId::LiveTableSearch => settings.live_table_search = !settings.live_table_search,
            SettingId::SqlHistorySize => {
                settings.sql_history_size =
                    cycle_usize(settings.sql_history_size, &[10, 25, 50, 100, 200]);
            }
            SettingId::SqlResultRowLimit => {
                settings.sql_result_row_limit = cycle_usize(
                    settings.sql_result_row_limit,
                    &[100, 200, 500, 1000, 2000, 5000, 10_000],
                );
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SettingsState {
    pub selected: usize,
    pub scroll_offset: usize,
}

impl SettingsState {
    pub fn new() -> Self {
        Self {
            selected: 0,
            scroll_offset: 0,
        }
    }

    pub fn selected_setting(&self) -> SettingId {
        let index = self.selected.min(SettingId::ALL.len().saturating_sub(1));
        SettingId::ALL[index]
    }
}

impl AppSettings {
    pub fn load() -> Result<Self> {
        SettingsStorage::load()
    }

    pub fn save(&self) -> Result<()> {
        SettingsStorage::save(self)
    }
}

impl super::App {
    pub fn settings_open(&self) -> bool {
        self.settings_page.is_some()
    }

    pub fn settings_selected_index(&self) -> Option<usize> {
        self.settings_page.as_ref().map(|state| state.selected)
    }

    pub fn settings_scroll_offset(&self, visible_rows: usize) -> usize {
        let Some(state) = &self.settings_page else {
            return 0;
        };
        if visible_rows == 0 || SettingId::ALL.len() <= visible_rows {
            return 0;
        }

        state
            .selected
            .saturating_sub(visible_rows.saturating_sub(1))
            .min(SettingId::ALL.len().saturating_sub(visible_rows))
    }

    pub fn open_settings(&mut self) {
        self.modal = None;
        self.filter_modal = None;
        self.detail = None;
        self.close_search();
        self.settings_page = Some(SettingsState::new());
    }

    pub fn close_settings(&mut self) -> Result<()> {
        if self.settings_page.is_some() {
            self.settings_page = None;
            self.app_settings.save()?;
            self.reload_recents_for_settings()?;
        }
        Ok(())
    }

    pub fn settings_lines(&self) -> Vec<(String, String, bool)> {
        let Some(state) = &self.settings_page else {
            return Vec::new();
        };

        SettingId::ALL
            .iter()
            .enumerate()
            .map(|(index, setting)| {
                let selected = index == state.selected;
                (
                    setting.label().to_string(),
                    setting.display_value(&self.app_settings),
                    selected,
                )
            })
            .collect()
    }

    pub fn settings_selected_description(&self) -> String {
        self.settings_page
            .as_ref()
            .map(|state| state.selected_setting().description().to_string())
            .unwrap_or_default()
    }

    pub fn settings_footer_hint(&self) -> String {
        "↑↓ move  Space change  Esc close".to_string()
    }

    pub fn handle_settings(&mut self, action: super::Action) -> Result<()> {
        if self.settings_page.is_none() {
            return Ok(());
        }

        let mut selected = self.settings_page.as_ref().map(|state| state.selected);
        let mut should_close = false;
        let mut reload_recents = false;

        match action {
            super::Action::MoveUp => {
                if let Some(index) = selected.as_mut()
                    && *index > 0
                {
                    *index -= 1;
                }
            }
            super::Action::MoveDown => {
                if let Some(index) = selected.as_mut()
                    && *index + 1 < SettingId::ALL.len()
                {
                    *index += 1;
                }
            }
            super::Action::ToggleItem | super::Action::Confirm => {
                if let Some(index) = selected {
                    let setting = SettingId::ALL[index];
                    setting.adjust(&mut self.app_settings);
                    self.app_settings.save()?;
                    reload_recents = matches!(setting, SettingId::RecentLimit);
                }
            }
            super::Action::CloseModal => should_close = true,
            _ => {}
        }

        if let Some(index) = selected {
            self.settings_page = Some(SettingsState {
                selected: index,
                scroll_offset: 0,
            });
        }

        if should_close {
            self.close_settings()?;
        } else if reload_recents {
            self.reload_recents_for_settings()?;
        }

        Ok(())
    }

    fn reload_recents_for_settings(&mut self) -> Result<()> {
        self.recent_items = super::RecentStore::load(self.app_settings.recent_limit)?;
        self.refresh_home_selection();
        Ok(())
    }
}

fn bool_label(value: bool) -> String {
    if value {
        "on".to_string()
    } else {
        "off".to_string()
    }
}

fn cycle_usize(current: usize, options: &[usize]) -> usize {
    let index = options
        .iter()
        .position(|value| *value == current)
        .unwrap_or(0);
    options[(index + 1) % options.len()]
}

fn cycle_u64(current: u64, options: &[u64]) -> u64 {
    let index = options
        .iter()
        .position(|value| *value == current)
        .unwrap_or(0);
    options[(index + 1) % options.len()]
}

#[cfg(test)]
mod tests;
