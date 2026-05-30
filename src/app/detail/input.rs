use anyhow::Result;

use super::super::{Action, App, DetailMessage, DetailPane};

impl App {
    pub(in crate::app) fn handle_detail(&mut self, action: Action) -> Result<()> {
        match action {
            Action::CloseModal | Action::Quit => self.detail = None,
            Action::ReverseFocus => self.detail_move_left(),
            Action::MoveLeft => self.detail_move_left(),
            Action::MoveRight | Action::ToggleFocus => self.detail_move_right(),
            Action::MoveUp => self.detail_move_up(),
            Action::MoveDown => self.detail_move_down(),
            Action::FollowLink | Action::Confirm => self.follow_detail_link()?,
            Action::EditDetail | Action::ToggleItem => self.toggle_detail_editing(),
            Action::SaveDetail => self.save_detail_changes()?,
            Action::DeleteRow => self.delete_selected_row()?,
            Action::DiscardDetail => self.discard_detail_changes(),
            Action::InputChar(ch) => self.detail_input_char(ch),
            Action::Backspace => self.detail_backspace(),
            Action::NewLine => self.detail_insert_newline(),
            Action::None
            | Action::NewRow
            | Action::SwitchToBrowse
            | Action::SwitchToSql
            | Action::ToggleView
            | Action::MoveHome
            | Action::MoveEnd
            | Action::PageUp
            | Action::PageDown
            | Action::OpenConfig
            | Action::Delete
            | Action::Clear
            | Action::Reload
            | Action::OpenSearchCurrent
            | Action::OpenSearchAll
            | Action::OpenFilters
            | Action::ExecuteSql => {}
        }

        Ok(())
    }

    pub(super) fn detail_move_left(&mut self) {
        if let Some(detail) = &mut self.detail {
            detail.pane = DetailPane::Fields;
            detail.is_editing = false;
        }
    }

    pub(super) fn detail_move_right(&mut self) {
        if let Some(detail) = &mut self.detail {
            detail.pane = DetailPane::Value;
        }
    }

    pub(super) fn detail_move_up(&mut self) {
        let Some(detail) = &mut self.detail else {
            return;
        };
        if detail.is_editing {
            return;
        }
        match detail.pane {
            DetailPane::Fields => {
                detail.selected_field = detail.selected_field.saturating_sub(1);
                detail.value_scroll = 0;
            }
            DetailPane::Value => {
                detail.value_scroll = detail.value_scroll.saturating_sub(1);
            }
        }
    }

    pub(super) fn detail_move_down(&mut self) {
        let Some(detail) = &mut self.detail else {
            return;
        };
        if detail.is_editing {
            return;
        }
        match detail.pane {
            DetailPane::Fields => {
                if !detail.fields.is_empty() {
                    detail.selected_field =
                        (detail.selected_field + 1).min(detail.fields.len().saturating_sub(1));
                    detail.value_scroll = 0;
                }
            }
            DetailPane::Value => {
                detail.value_scroll = detail.value_scroll.saturating_add(1);
            }
        }
        self.clamp_detail_scroll();
    }

    fn toggle_detail_editing(&mut self) {
        let database_is_writable = self.detail_database_is_writable();
        let Some(detail) = &mut self.detail else {
            return;
        };

        detail.message = None;
        if detail.is_editing {
            detail.is_editing = false;
            detail.pane = DetailPane::Value;
            self.clamp_detail_scroll();
            return;
        }

        let Some(field) = detail.fields.get(detail.selected_field) else {
            return;
        };

        if detail.rowid.is_none() && !detail.is_new_row {
            detail.message = Some(DetailMessage {
                text: "This row is read-only and cannot be edited".to_string(),
                is_error: true,
            });
            return;
        }
        if !database_is_writable {
            detail.message = Some(DetailMessage {
                text: "This database is read-only and cannot be edited".to_string(),
                is_error: true,
            });
            return;
        }
        if field.is_blob {
            detail.message = Some(DetailMessage {
                text: "Blob values are read-only in the details modal".to_string(),
                is_error: true,
            });
            return;
        }

        detail.pane = DetailPane::Value;
        detail.is_editing = true;
        detail.value_scroll = 0;
        self.clamp_detail_scroll();
    }

    fn detail_input_char(&mut self, ch: char) {
        let Some(detail) = &mut self.detail else {
            return;
        };
        if !detail.is_editing {
            return;
        }

        if let Some(field) = detail.fields.get_mut(detail.selected_field) {
            field.draft_value.push(ch);
            detail.message = None;
        }
        self.clamp_detail_scroll();
    }

    fn detail_backspace(&mut self) {
        let Some(detail) = &mut self.detail else {
            return;
        };
        if !detail.is_editing {
            return;
        }

        if let Some(field) = detail.fields.get_mut(detail.selected_field) {
            field.draft_value.pop();
            detail.message = None;
        }
        self.clamp_detail_scroll();
    }

    fn detail_insert_newline(&mut self) {
        let Some(detail) = &mut self.detail else {
            return;
        };
        if !detail.is_editing {
            return;
        }

        if let Some(field) = detail.fields.get_mut(detail.selected_field) {
            field.draft_value.push('\n');
            detail.message = None;
        }
        self.clamp_detail_scroll();
    }
}
