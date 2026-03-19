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
#[path = "../testing/runtime/input.rs"]
mod tests;
