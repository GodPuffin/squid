use crossterm::event::KeyCode;

use crate::app::{Action, App, FilterPane};

pub fn action_for_key(app: &App, key: KeyCode) -> Action {
    if app.detail.is_some() {
        return detail_action(key);
    }
    if app.search.is_some() {
        return search_action(key);
    }
    if app.filter_modal.is_some() {
        return filter_action(app, key);
    }
    if app.modal.is_some() {
        return modal_action(key);
    }
    root_action(key)
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
