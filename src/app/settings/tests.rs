use super::{AppSettings, DefaultBrowseView, SettingId};

#[test]
fn setting_ids_cycle_numeric_options() {
    let mut settings = AppSettings::default();
    assert_eq!(settings.recent_limit, 10);

    SettingId::RecentLimit.adjust(&mut settings);
    assert_eq!(settings.recent_limit, 15);

    SettingId::SqlResultRowLimit.adjust(&mut settings);
    assert_eq!(settings.sql_result_row_limit, 500);

    SettingId::DoubleClickIntervalMs.adjust(&mut settings);
    assert_eq!(settings.double_click_interval_ms, 750);

    SettingId::CellPreviewMaxChars.adjust(&mut settings);
    assert_eq!(settings.cell_preview_max_chars, 400);
}

#[test]
fn setting_ids_toggle_booleans() {
    let mut settings = AppSettings::default();
    assert!(settings.mouse_enabled);

    SettingId::MouseEnabled.adjust(&mut settings);
    assert!(!settings.mouse_enabled);

    SettingId::ConfirmBeforeRemoveRecent.adjust(&mut settings);
    assert!(!settings.confirm_before_remove_recent);
}

#[test]
fn default_browse_view_toggles() {
    let mut settings = AppSettings::default();
    assert_eq!(settings.default_browse_view, DefaultBrowseView::Rows);

    SettingId::DefaultBrowseView.adjust(&mut settings);
    assert_eq!(settings.default_browse_view, DefaultBrowseView::Schema);
}
