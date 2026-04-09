use anyhow::Result;

use super::{Action, App, ModalPane, SortRule};

impl App {
    pub fn modal_click_columns(&mut self, index: usize) -> Result<()> {
        if let Some(modal) = &mut self.modal {
            modal.pane = ModalPane::Columns;
            modal.column_index = index;
        }
        self.modal_toggle_column(index)
    }

    pub fn modal_click_sort_candidate(&mut self, index: usize, descending: bool) -> Result<()> {
        if let Some(modal) = &mut self.modal {
            modal.pane = ModalPane::SortColumns;
            modal.sort_column_index = index;
            modal.pending_desc = descending;
        }
        self.modal_confirm_sort()
    }

    pub fn modal_select_sort_rule(&mut self, index: usize) {
        if let Some(modal) = &mut self.modal {
            modal.pane = ModalPane::SortActive;
            modal.sort_active_index = index;
        }
    }

    pub fn modal_remove_sort_rule(&mut self, index: usize) -> Result<()> {
        if let Some(modal) = &mut self.modal {
            modal.pane = ModalPane::SortActive;
            modal.sort_active_index = index;
        }
        self.modal_delete_sort()
    }

    pub(super) fn handle_modal(&mut self, action: Action) -> Result<()> {
        match action {
            Action::CloseModal | Action::Quit | Action::OpenConfig => {
                self.modal = None;
            }
            Action::OpenFilters => {
                self.modal = None;
                self.open_filter_modal();
            }
            Action::ReverseFocus => self.modal_move_left(),
            Action::MoveLeft => self.modal_move_left(),
            Action::MoveRight | Action::ToggleFocus => self.modal_move_right(),
            Action::MoveUp => self.modal_move_up(),
            Action::MoveDown => self.modal_move_down(),
            Action::ToggleItem => self.modal_toggle_current_item()?,
            Action::Confirm => self.modal_confirm_sort()?,
            Action::Delete => self.modal_delete_sort()?,
            Action::Clear => self.modal_clear_sorts()?,
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
            | Action::InputChar(_)
            | Action::Backspace
            | Action::ExecuteSql
            | Action::NewLine => {}
        }

        Ok(())
    }

    pub(super) fn open_config_modal(&mut self) {
        self.modal = Some(super::ModalState {
            pane: ModalPane::Columns,
            column_index: 0,
            sort_column_index: 0,
            sort_active_index: 0,
            pending_desc: false,
        });
    }

    fn modal_move_left(&mut self) {
        if let Some(modal) = &mut self.modal {
            modal.pane = match modal.pane {
                ModalPane::Columns => ModalPane::SortActive,
                ModalPane::SortColumns => ModalPane::Columns,
                ModalPane::SortActive => ModalPane::SortColumns,
            };
        }
    }

    fn modal_move_right(&mut self) {
        if let Some(modal) = &mut self.modal {
            modal.pane = match modal.pane {
                ModalPane::Columns => ModalPane::SortColumns,
                ModalPane::SortColumns => ModalPane::SortActive,
                ModalPane::SortActive => ModalPane::Columns,
            };
        }
    }

    fn modal_move_up(&mut self) {
        let column_count = self.details.as_ref().map(|d| d.columns.len()).unwrap_or(0);
        let sort_len = self.current_sort_rules().len();

        if let Some(modal) = &mut self.modal {
            match modal.pane {
                ModalPane::Columns => modal.column_index = modal.column_index.saturating_sub(1),
                ModalPane::SortColumns => {
                    modal.sort_column_index = modal.sort_column_index.saturating_sub(1)
                }
                ModalPane::SortActive => {
                    if sort_len > 0 {
                        modal.sort_active_index = modal.sort_active_index.saturating_sub(1);
                    }
                }
            }

            if column_count > 0 {
                modal.column_index = modal.column_index.min(column_count.saturating_sub(1));
                modal.sort_column_index =
                    modal.sort_column_index.min(column_count.saturating_sub(1));
            }
        }
    }

    fn modal_move_down(&mut self) {
        let column_count = self.details.as_ref().map(|d| d.columns.len()).unwrap_or(0);
        let sort_len = self.current_sort_rules().len();

        if let Some(modal) = &mut self.modal {
            match modal.pane {
                ModalPane::Columns => {
                    if column_count > 0 {
                        modal.column_index =
                            (modal.column_index + 1).min(column_count.saturating_sub(1));
                    }
                }
                ModalPane::SortColumns => {
                    if column_count > 0 {
                        modal.sort_column_index =
                            (modal.sort_column_index + 1).min(column_count.saturating_sub(1));
                    }
                }
                ModalPane::SortActive => {
                    if sort_len > 0 {
                        modal.sort_active_index =
                            (modal.sort_active_index + 1).min(sort_len.saturating_sub(1));
                    }
                }
            }
        }
    }

    fn modal_toggle_current_item(&mut self) -> Result<()> {
        let Some(pane) = self.modal.as_ref().map(|modal| modal.pane) else {
            return Ok(());
        };

        match pane {
            ModalPane::Columns => {
                let index = self
                    .modal
                    .as_ref()
                    .map(|modal| modal.column_index)
                    .unwrap_or(0);
                self.modal_toggle_column(index)?;
            }
            ModalPane::SortColumns => {
                if let Some(modal) = &mut self.modal {
                    modal.pending_desc = !modal.pending_desc;
                }
            }
            ModalPane::SortActive => {
                let Some(index) = self.modal.as_ref().map(|modal| modal.sort_active_index) else {
                    return Ok(());
                };
                if let Some(config) = self.current_config_mut()
                    && let Some(rule) = config.sort_clauses.get_mut(index)
                {
                    rule.descending = !rule.descending;
                    self.refresh_preview()?;
                }
            }
        }

        Ok(())
    }

    fn modal_toggle_column(&mut self, index: usize) -> Result<()> {
        if let Some(config) = self.current_config_mut()
            && index < config.visible_columns.len()
        {
            let visible_count = config
                .visible_columns
                .iter()
                .filter(|visible| **visible)
                .count();
            if config.visible_columns[index] && visible_count == 1 {
                return Ok(());
            }
            config.visible_columns[index] = !config.visible_columns[index];
            self.refresh_preview()?;
        }
        Ok(())
    }

    fn modal_confirm_sort(&mut self) -> Result<()> {
        let Some(modal) = &self.modal else {
            return Ok(());
        };

        let column_index = modal.sort_column_index;
        let descending = modal.pending_desc;

        if let Some(config) = self.current_config_mut() {
            if let Some(existing) = config
                .sort_clauses
                .iter_mut()
                .find(|rule| rule.column_index == column_index)
            {
                existing.descending = descending;
            } else {
                config.sort_clauses.push(SortRule {
                    column_index,
                    descending,
                });
            }
            self.refresh_preview()?;
        }

        Ok(())
    }

    fn modal_delete_sort(&mut self) -> Result<()> {
        let Some(index) = self.modal.as_ref().map(|modal| modal.sort_active_index) else {
            return Ok(());
        };

        let new_len = if let Some(config) = self.current_config_mut() {
            if index < config.sort_clauses.len() {
                config.sort_clauses.remove(index);
                Some(config.sort_clauses.len())
            } else {
                None
            }
        } else {
            None
        };

        if let Some(len) = new_len
            && let Some(modal) = &mut self.modal
        {
            modal.sort_active_index = modal.sort_active_index.min(len.saturating_sub(1));
        }

        if new_len.is_some() {
            self.refresh_preview()?;
        }

        Ok(())
    }

    fn modal_clear_sorts(&mut self) -> Result<()> {
        let changed = if let Some(config) = self.current_config_mut() {
            if config.sort_clauses.is_empty() {
                false
            } else {
                config.sort_clauses.clear();
                true
            }
        } else {
            false
        };

        if changed {
            if let Some(modal) = &mut self.modal {
                modal.sort_active_index = 0;
            }
            self.refresh_preview()?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests;
