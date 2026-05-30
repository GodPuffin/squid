use super::{AppSettings, SettingId};

#[test]
fn settings_footer_hint_does_not_imply_q_quits_app() {
    let mut app = super::super::App::load(None).expect("load app");
    app.open_settings();
    let hint = app.settings_footer_hint();
    assert!(hint.contains("Esc/q close"));
    assert!(!hint.contains("q quit"));
}

#[test]
fn setting_ids_cycle_numeric_options() {
    let mut settings = AppSettings::default();
    assert_eq!(settings.recent_limit, 10);

    SettingId::RecentLimit.adjust(&mut settings);
    assert_eq!(settings.recent_limit, 15);

    SettingId::SqlResultRowLimit.adjust(&mut settings);
    assert_eq!(settings.sql_result_row_limit, 500);
}

#[test]
fn setting_ids_toggle_booleans() {
    let mut settings = AppSettings::default();
    assert!(settings.mouse_enabled);

    SettingId::MouseEnabled.adjust(&mut settings);
    assert!(!settings.mouse_enabled);
}
