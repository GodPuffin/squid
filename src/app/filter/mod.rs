use anyhow::Result;

use crate::db::FilterMode;

use super::{Action, App, FilterModalState, FilterPane, FilterRule};

impl App {
    pub fn filter_modal_select_column(&mut self, index: usize) {
        if let Some(modal) = &mut self.filter_modal {
            modal.pane = FilterPane::Columns;
            modal.column_index = index;
        }
        self.sync_filter_modal_draft();
    }

    pub fn filter_modal_select_mode(&mut self, index: usize) {
        if let Some(modal) = &mut self.filter_modal {
            modal.pane = FilterPane::Modes;
            modal.mode_index = index;
        }
    }

    pub fn filter_modal_select_active(&mut self, index: usize) {
        if let Some(modal) = &mut self.filter_modal {
            modal.pane = FilterPane::Active;
            modal.active_index = index;
        }
    }

    pub fn filter_modal_focus_draft(&mut self) {
        if let Some(modal) = &mut self.filter_modal {
            modal.pane = FilterPane::Draft;
        }
    }

    pub(super) fn open_filter_modal(&mut self) {
        self.filter_modal = Some(FilterModalState {
            pane: FilterPane::Columns,
            column_index: 0,
            mode_index: 0,
            active_index: 0,
            input: String::new(),
        });
        self.sync_filter_modal_draft();
    }

    pub(super) fn handle_filter_modal(&mut self, action: Action) -> Result<()> {
        match action {
            Action::CloseModal | Action::Quit | Action::OpenFilters => {
                self.filter_modal = None;
            }
            Action::OpenConfig => {
                self.filter_modal = None;
                self.open_config_modal();
            }
            Action::ReverseFocus => self.filter_modal_move_left(),
            Action::MoveLeft => self.filter_modal_move_left(),
            Action::MoveRight | Action::ToggleFocus => self.filter_modal_move_right(),
            Action::MoveUp => self.filter_modal_move_up(),
            Action::MoveDown => self.filter_modal_move_down(),
            Action::ToggleItem => self.filter_modal_toggle_current_item()?,
            Action::Confirm => self.apply_filter_modal_rule()?,
            Action::Delete => self.delete_filter_modal_rule()?,
            Action::Clear => self.clear_all_filters()?,
            Action::InputChar(ch) => self.filter_modal_input_char(ch),
            Action::Backspace => self.filter_modal_backspace(),
            Action::None
            | Action::SwitchToBrowse
            | Action::SwitchToSql
            | Action::ToggleView
            | Action::MoveHome
            | Action::MoveEnd
            | Action::PageUp
            | Action::PageDown
            | Action::Reload
            | Action::OpenSearchCurrent
            | Action::OpenSearchAll
            | Action::FollowLink
            | Action::EditDetail
            | Action::SaveDetail
            | Action::DiscardDetail
            | Action::ExecuteSql
            | Action::NewLine => {}
        }

        Ok(())
    }

    pub fn active_filter_mode(&self) -> Option<FilterMode> {
        let modal = self.filter_modal.as_ref()?;
        self.available_filter_modes_for_column(modal.column_index)
            .get(modal.mode_index)
            .copied()
    }

    pub fn filter_modal_mode_lines(&self) -> Vec<String> {
        let Some(modal) = &self.filter_modal else {
            return vec![];
        };

        self.available_filter_modes_for_column(modal.column_index)
            .into_iter()
            .map(filter_mode_name)
            .map(str::to_string)
            .collect()
    }

    pub fn filter_modal_active_lines(&self) -> Vec<String> {
        self.modal_filter_active_lines()
    }

    pub fn filter_modal_selected_indices(&self) -> (Option<usize>, Option<usize>, Option<usize>) {
        let Some(modal) = &self.filter_modal else {
            return (None, None, None);
        };
        let active_len = self.current_filter_rules().len();
        let active_index = if active_len == 0 {
            None
        } else {
            Some(modal.active_index.min(active_len.saturating_sub(1)))
        };

        (
            self.details.as_ref().map(|_| modal.column_index),
            Some(modal.mode_index),
            active_index,
        )
    }

    fn filter_modal_move_left(&mut self) {
        if let Some(modal) = &mut self.filter_modal {
            modal.pane = match modal.pane {
                FilterPane::Columns => FilterPane::Active,
                FilterPane::Modes => FilterPane::Columns,
                FilterPane::Draft => FilterPane::Modes,
                FilterPane::Active => FilterPane::Draft,
            };
        }
    }

    fn filter_modal_move_right(&mut self) {
        if let Some(modal) = &mut self.filter_modal {
            modal.pane = match modal.pane {
                FilterPane::Columns => FilterPane::Modes,
                FilterPane::Modes => FilterPane::Draft,
                FilterPane::Draft => FilterPane::Active,
                FilterPane::Active => FilterPane::Columns,
            };
        }
    }

    fn filter_modal_move_up(&mut self) {
        let Some(pane) = self.filter_modal.as_ref().map(|modal| modal.pane) else {
            return;
        };

        match pane {
            FilterPane::Columns => {
                if let Some(modal) = &mut self.filter_modal {
                    modal.column_index = modal.column_index.saturating_sub(1);
                }
                self.sync_filter_modal_draft();
            }
            FilterPane::Modes => {
                if let Some(modal) = &mut self.filter_modal {
                    modal.mode_index = modal.mode_index.saturating_sub(1);
                }
            }
            FilterPane::Draft => {}
            FilterPane::Active => {
                if let Some(modal) = &mut self.filter_modal {
                    modal.active_index = modal.active_index.saturating_sub(1);
                }
            }
        }
    }

    fn filter_modal_move_down(&mut self) {
        let column_count = self.details.as_ref().map(|d| d.columns.len()).unwrap_or(0);
        let active_len = self.current_filter_rules().len();
        let Some(pane) = self.filter_modal.as_ref().map(|modal| modal.pane) else {
            return;
        };

        match pane {
            FilterPane::Columns => {
                if let Some(modal) = &mut self.filter_modal
                    && column_count > 0
                {
                    modal.column_index =
                        (modal.column_index + 1).min(column_count.saturating_sub(1));
                }
                self.sync_filter_modal_draft();
            }
            FilterPane::Modes => {
                let mode_count = self
                    .filter_modal
                    .as_ref()
                    .map(|modal| {
                        self.available_filter_modes_for_column(modal.column_index)
                            .len()
                    })
                    .unwrap_or(0);
                if let Some(modal) = &mut self.filter_modal
                    && mode_count > 0
                {
                    modal.mode_index = (modal.mode_index + 1).min(mode_count.saturating_sub(1));
                }
            }
            FilterPane::Draft => {}
            FilterPane::Active => {
                if let Some(modal) = &mut self.filter_modal
                    && active_len > 0
                {
                    modal.active_index = (modal.active_index + 1).min(active_len.saturating_sub(1));
                }
            }
        }
    }

    fn filter_modal_toggle_current_item(&mut self) -> Result<()> {
        let Some(pane) = self.filter_modal.as_ref().map(|modal| modal.pane) else {
            return Ok(());
        };

        match pane {
            FilterPane::Columns => {
                let index = self
                    .filter_modal
                    .as_ref()
                    .map(|modal| modal.column_index)
                    .unwrap_or(0);
                self.modal_toggle_column(index)?;
            }
            FilterPane::Modes | FilterPane::Draft => self.filter_modal_cycle_mode(),
            FilterPane::Active => {}
        }

        Ok(())
    }

    fn filter_modal_cycle_mode(&mut self) {
        let Some(pane) = self.filter_modal.as_ref().map(|modal| modal.pane) else {
            return;
        };
        if pane == FilterPane::Draft
            && filter_mode_uses_input(self.active_filter_mode().unwrap_or(FilterMode::Contains))
        {
            if let Some(modal) = &mut self.filter_modal {
                modal.input.push(' ');
            }
            return;
        }
        if !matches!(pane, FilterPane::Modes | FilterPane::Draft) {
            return;
        }

        let mode_count = self
            .filter_modal
            .as_ref()
            .map(|modal| {
                self.available_filter_modes_for_column(modal.column_index)
                    .len()
            })
            .unwrap_or(0);
        if let Some(modal) = &mut self.filter_modal
            && mode_count > 0
        {
            modal.mode_index = (modal.mode_index + 1) % mode_count;
        }
    }

    fn filter_modal_input_char(&mut self, ch: char) {
        if self.filter_modal_pane() != Some(FilterPane::Draft) {
            return;
        }
        let uses_input =
            filter_mode_uses_input(self.active_filter_mode().unwrap_or(FilterMode::Contains));
        if let Some(modal) = &mut self.filter_modal
            && uses_input
        {
            modal.input.push(ch);
        }
    }

    fn filter_modal_backspace(&mut self) {
        if self.filter_modal_pane() != Some(FilterPane::Draft) {
            return;
        }
        let uses_input =
            filter_mode_uses_input(self.active_filter_mode().unwrap_or(FilterMode::Contains));
        if let Some(modal) = &mut self.filter_modal
            && uses_input
        {
            modal.input.pop();
        }
    }

    fn apply_filter_modal_rule(&mut self) -> Result<()> {
        let Some((column_index, input)) = self
            .filter_modal
            .as_ref()
            .map(|modal| (modal.column_index, modal.input.clone()))
        else {
            return Ok(());
        };
        let Some(mode) = self.active_filter_mode() else {
            return Ok(());
        };

        let value = if filter_mode_uses_input(mode) {
            let trimmed = input.trim().to_string();
            if trimmed.is_empty() {
                return Ok(());
            }
            trimmed
        } else {
            String::new()
        };

        if let Some(config) = self.current_config_mut() {
            config
                .filter_rules
                .retain(|rule| rule.column_index != column_index);
            config.filter_rules.push(FilterRule {
                column_index,
                mode,
                value,
            });
            self.reset_content_position();
            self.refresh_preview()?;
        }

        Ok(())
    }

    fn delete_filter_modal_rule(&mut self) -> Result<()> {
        let Some((pane, column_index, active_index)) = self
            .filter_modal
            .as_ref()
            .map(|modal| (modal.pane, modal.column_index, modal.active_index))
        else {
            return Ok(());
        };

        let target_column = if pane == FilterPane::Active {
            self.current_filter_rules()
                .get(active_index)
                .map(|rule| rule.column_index)
                .unwrap_or(column_index)
        } else {
            column_index
        };

        let mut changed = false;
        if let Some(config) = self.current_config_mut() {
            let before = config.filter_rules.len();
            config
                .filter_rules
                .retain(|rule| rule.column_index != target_column);
            changed = config.filter_rules.len() != before;
        }

        if changed {
            self.reset_content_position();
            self.refresh_preview()?;
        }
        self.sync_filter_modal_draft();

        Ok(())
    }

    fn clear_all_filters(&mut self) -> Result<()> {
        let changed = if let Some(config) = self.current_config_mut() {
            if config.filter_rules.is_empty() {
                false
            } else {
                config.filter_rules.clear();
                true
            }
        } else {
            false
        };

        if changed {
            self.reset_content_position();
            self.refresh_preview()?;
        }
        self.sync_filter_modal_draft();

        Ok(())
    }

    fn sync_filter_modal_draft(&mut self) {
        let Some(column_index) = self.filter_modal.as_ref().map(|modal| modal.column_index) else {
            return;
        };

        let modes = self.available_filter_modes_for_column(column_index);
        let existing = self
            .current_filter_rules()
            .into_iter()
            .find(|rule| rule.column_index == column_index);
        let active_len = self.current_filter_rules().len();

        if let Some(modal) = &mut self.filter_modal {
            if let Some(rule) = existing {
                modal.mode_index = modes
                    .iter()
                    .position(|mode| *mode == rule.mode)
                    .unwrap_or(0);
                modal.input = rule.value;
            } else {
                modal.mode_index = 0;
                modal.input.clear();
            }

            if !modes.is_empty() {
                modal.mode_index = modal.mode_index.min(modes.len().saturating_sub(1));
            }
            if active_len > 0 {
                modal.active_index = modal.active_index.min(active_len.saturating_sub(1));
            } else {
                modal.active_index = 0;
            }
        }
    }

    fn available_filter_modes_for_column(&self, column_index: usize) -> Vec<FilterMode> {
        match self.column_filter_kind(column_index) {
            ColumnFilterKind::Boolean => vec![FilterMode::IsTrue, FilterMode::IsFalse],
            ColumnFilterKind::Numeric => {
                vec![
                    FilterMode::Equals,
                    FilterMode::GreaterThan,
                    FilterMode::LessThan,
                ]
            }
            ColumnFilterKind::Text => {
                vec![
                    FilterMode::Contains,
                    FilterMode::Equals,
                    FilterMode::StartsWith,
                ]
            }
        }
    }

    fn column_filter_kind(&self, column_index: usize) -> ColumnFilterKind {
        let data_type = self
            .details
            .as_ref()
            .and_then(|details| details.columns.get(column_index))
            .map(|column| column.data_type.to_ascii_uppercase())
            .unwrap_or_default();

        if data_type.contains("BOOL") {
            ColumnFilterKind::Boolean
        } else if data_type.contains("INT")
            || data_type.contains("REAL")
            || data_type.contains("FLOA")
            || data_type.contains("DOUB")
            || data_type.contains("NUM")
            || data_type.contains("DEC")
        {
            ColumnFilterKind::Numeric
        } else {
            ColumnFilterKind::Text
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ColumnFilterKind {
    Text,
    Numeric,
    Boolean,
}

fn filter_mode_name(mode: FilterMode) -> &'static str {
    match mode {
        FilterMode::Contains => "contains",
        FilterMode::Equals => "equals",
        FilterMode::StartsWith => "starts with",
        FilterMode::GreaterThan => "greater than",
        FilterMode::LessThan => "less than",
        FilterMode::IsTrue => "is true",
        FilterMode::IsFalse => "is false",
    }
}

fn filter_mode_uses_input(mode: FilterMode) -> bool {
    !matches!(mode, FilterMode::IsTrue | FilterMode::IsFalse)
}

#[cfg(test)]
mod tests;
