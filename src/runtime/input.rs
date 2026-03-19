use crossterm::event::{KeyCode, KeyEvent};

use crate::app::{Action, App, AppMode, FilterPane, SqlPane};

pub fn action_for_key(app: &App, key: KeyEvent) -> Action {
    if app.mode == AppMode::Sql {
        return sql_action(app, key.code);
    }

    if app.detail.is_some() {
        return detail_action(key.code);
    }
    if app.search.is_some() {
        return search_action(key.code);
    }
    if app.filter_modal.is_some() {
        return filter_action(app, key.code);
    }
    if app.modal.is_some() {
        return modal_action(key.code);
    }
    root_action(key.code)
}

fn detail_action(key: KeyCode) -> Action {
    match key {
        KeyCode::Esc | KeyCode::Char('q') => Action::CloseModal,
        KeyCode::Up => Action::MoveUp,
        KeyCode::Down => Action::MoveDown,
        KeyCode::Left => Action::MoveLeft,
        KeyCode::Right => Action::MoveRight,
        KeyCode::Char('g') => Action::FollowLink,
        KeyCode::Enter => Action::Confirm,
        _ => Action::None,
    }
}

fn search_action(key: KeyCode) -> Action {
    match key {
        KeyCode::BackTab => Action::ReverseFocus,
        KeyCode::Esc => Action::CloseModal,
        KeyCode::Up => Action::MoveUp,
        KeyCode::Down => Action::MoveDown,
        KeyCode::Enter => Action::Confirm,
        KeyCode::Backspace => Action::Backspace,
        KeyCode::Char(ch) if !ch.is_control() => Action::InputChar(ch),
        _ => Action::None,
    }
}

fn filter_action(app: &App, key: KeyCode) -> Action {
    match key {
        KeyCode::BackTab => Action::ReverseFocus,
        KeyCode::Esc => Action::CloseModal,
        KeyCode::Char('q') if app.filter_modal_pane() != Some(FilterPane::Draft) => {
            Action::CloseModal
        }
        KeyCode::Tab => Action::ToggleFocus,
        KeyCode::Up => Action::MoveUp,
        KeyCode::Down => Action::MoveDown,
        KeyCode::Left => Action::MoveLeft,
        KeyCode::Right => Action::MoveRight,
        KeyCode::Char(' ') => Action::ToggleItem,
        KeyCode::Enter => Action::Confirm,
        KeyCode::Delete => Action::Delete,
        KeyCode::Backspace => Action::Backspace,
        KeyCode::Char('c') => Action::Clear,
        KeyCode::Char(ch) if !ch.is_control() => Action::InputChar(ch),
        _ => Action::None,
    }
}

fn modal_action(key: KeyCode) -> Action {
    match key {
        KeyCode::BackTab => Action::ReverseFocus,
        KeyCode::Esc | KeyCode::Char('q') => Action::CloseModal,
        KeyCode::Tab => Action::ToggleFocus,
        KeyCode::Up => Action::MoveUp,
        KeyCode::Down => Action::MoveDown,
        KeyCode::Left => Action::MoveLeft,
        KeyCode::Right => Action::MoveRight,
        KeyCode::Char(' ') => Action::ToggleItem,
        KeyCode::Enter => Action::Confirm,
        KeyCode::Delete => Action::Delete,
        KeyCode::Backspace => Action::Backspace,
        KeyCode::Char('c') => Action::Clear,
        KeyCode::Char('M') | KeyCode::Char('f') => Action::OpenFilters,
        KeyCode::Char(ch) if !ch.is_control() => Action::InputChar(ch),
        _ => Action::None,
    }
}

fn root_action(key: KeyCode) -> Action {
    match key {
        KeyCode::Char('1') => Action::SwitchToBrowse,
        KeyCode::Char('2') => Action::SwitchToSql,
        KeyCode::BackTab => Action::ReverseFocus,
        KeyCode::Char('q') => Action::Quit,
        KeyCode::Esc => Action::CloseModal,
        KeyCode::Tab => Action::ToggleFocus,
        KeyCode::Char('v') => Action::ToggleView,
        KeyCode::Up => Action::MoveUp,
        KeyCode::Down => Action::MoveDown,
        KeyCode::Left => Action::MoveLeft,
        KeyCode::Right => Action::MoveRight,
        KeyCode::Char('m') => Action::OpenConfig,
        KeyCode::Char('M') => Action::OpenFilters,
        KeyCode::Char('f') => Action::OpenSearchCurrent,
        KeyCode::Char('F') => Action::OpenSearchAll,
        KeyCode::Char(' ') => Action::ToggleItem,
        KeyCode::Enter => Action::Confirm,
        KeyCode::Delete | KeyCode::Backspace => Action::Delete,
        KeyCode::Char('c') => Action::Clear,
        KeyCode::Char('r') => Action::Reload,
        _ => Action::None,
    }
}

fn sql_action(app: &App, key: KeyCode) -> Action {
    match key {
        KeyCode::Char('1') if app.sql_focus() != SqlPane::Editor => Action::SwitchToBrowse,
        KeyCode::Char('2') if app.sql_focus() != SqlPane::Editor => Action::SwitchToSql,
        KeyCode::BackTab => Action::ReverseFocus,
        KeyCode::Char('q') if app.sql_focus() != SqlPane::Editor => Action::Quit,
        KeyCode::Esc => Action::CloseModal,
        KeyCode::Tab => Action::ToggleFocus,
        KeyCode::F(5) => Action::ExecuteSql,
        KeyCode::Up => Action::MoveUp,
        KeyCode::Down => Action::MoveDown,
        KeyCode::Left => Action::MoveLeft,
        KeyCode::Right => Action::MoveRight,
        KeyCode::Home => Action::MoveHome,
        KeyCode::End => Action::MoveEnd,
        KeyCode::PageUp => Action::PageUp,
        KeyCode::PageDown => Action::PageDown,
        KeyCode::Enter => Action::NewLine,
        KeyCode::Delete => Action::Delete,
        KeyCode::Backspace => Action::Backspace,
        KeyCode::Char('c') if app.sql_focus() != SqlPane::Editor => Action::Clear,
        KeyCode::Char(ch) if !ch.is_control() => Action::InputChar(ch),
        _ => Action::None,
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use crossterm::event::{KeyCode, KeyEvent};
    use rusqlite::Connection;

    use super::action_for_key;
    use crate::app::{
        Action, App, AppMode, FilterModalState, FilterPane, SearchScope, SearchState,
    };

    #[test]
    fn root_digit_shortcuts_still_switch_modes() {
        let app = test_app("root-digit");

        assert_eq!(
            action_for_key(&app, KeyEvent::from(KeyCode::Char('1'))),
            Action::SwitchToBrowse
        );
        assert_eq!(
            action_for_key(&app, KeyEvent::from(KeyCode::Char('2'))),
            Action::SwitchToSql
        );
    }

    #[test]
    fn search_accepts_numeric_input() {
        let mut app = test_app("search-digit");
        app.search = Some(SearchState {
            scope: SearchScope::CurrentTable,
            query: String::new(),
            results: Vec::new(),
            selected_result: 0,
            result_offset: 0,
            result_limit: 10,
            submitted: false,
        });

        assert_eq!(
            action_for_key(&app, KeyEvent::from(KeyCode::Char('1'))),
            Action::InputChar('1')
        );
    }

    #[test]
    fn filter_draft_accepts_numeric_input() {
        let mut app = test_app("filter-digit");
        app.filter_modal = Some(FilterModalState {
            pane: FilterPane::Draft,
            column_index: 0,
            mode_index: 0,
            active_index: 0,
            input: String::new(),
        });

        assert_eq!(
            action_for_key(&app, KeyEvent::from(KeyCode::Char('2'))),
            Action::InputChar('2')
        );
    }

    #[test]
    fn sql_editor_accepts_q_and_digits_as_text() {
        let mut app = test_app("sql-editor-input");
        app.mode = AppMode::Sql;

        assert_eq!(
            action_for_key(&app, KeyEvent::from(KeyCode::Char('q'))),
            Action::InputChar('q')
        );
        assert_eq!(
            action_for_key(&app, KeyEvent::from(KeyCode::Char('1'))),
            Action::InputChar('1')
        );
    }

    fn test_app(label: &str) -> App {
        let path = temp_db_path(label);
        let conn = Connection::open(&path).expect("create db");
        conn.execute("CREATE TABLE demo(id INTEGER PRIMARY KEY, name TEXT)", [])
            .expect("create table");
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
        std::env::temp_dir().join(format!("squid-input-{label}-{stamp}.sqlite"))
    }
}
