mod completion;
mod cursor;
mod execution;
mod input;

use anyhow::Result;

use self::cursor::{index_for_line_col, line_col_from_index};
use super::{
    Action, App, SqlCompletionItem, SqlCompletionState, SqlHistoryEntry, SqlPane, SqlResultState,
};

impl App {
    pub fn set_sql_viewport_sizes(
        &mut self,
        editor_height: usize,
        editor_width: usize,
        history_height: usize,
        result_height: usize,
    ) {
        self.sql.editor_height = editor_height.max(1);
        self.sql.editor_width = editor_width.max(1);
        self.sql.history_height = history_height.max(1);
        self.sql.result_height = result_height.max(1);
        self.ensure_sql_viewport();
    }

    pub fn sql_focus(&self) -> SqlPane {
        self.sql.focus
    }

    pub fn sql_query_lines(&self) -> Vec<String> {
        if self.sql.query.is_empty() {
            vec![String::new()]
        } else {
            self.sql.query.lines().map(str::to_string).collect()
        }
    }

    pub fn sql_visible_history(&self) -> &[SqlHistoryEntry] {
        let len = self.sql.history.len();
        let end = (self.sql.history_offset + self.sql.history_height).min(len);
        &self.sql.history[self.sql.history_offset.min(len)..end]
    }

    pub fn sql_selected_history_in_view(&self) -> Option<usize> {
        if self.sql.history.is_empty() {
            None
        } else {
            self.sql
                .selected_history
                .checked_sub(self.sql.history_offset)
                .filter(|index| *index < self.sql_visible_history().len())
        }
    }

    pub fn sql_result_rows_in_view(&self) -> &[Vec<String>] {
        match &self.sql.result {
            SqlResultState::Rows { rows, .. } => {
                let len = rows.len();
                let end = (self.sql.result_scroll + self.sql.result_height).min(len);
                &rows[self.sql.result_scroll.min(len)..end]
            }
            SqlResultState::Empty | SqlResultState::Message { .. } => &[],
        }
    }

    pub fn sql_result_columns(&self) -> &[String] {
        match &self.sql.result {
            SqlResultState::Rows { columns, .. } => columns,
            SqlResultState::Empty | SqlResultState::Message { .. } => &[],
        }
    }

    pub fn sql_completion_items(&self) -> &[SqlCompletionItem] {
        self.sql
            .completion
            .as_ref()
            .map(|completion| completion.items.as_slice())
            .unwrap_or(&[])
    }

    pub fn sql_completion_window(&self, visible_items: usize) -> Option<(usize, usize, usize)> {
        let completion = self.sql.completion.as_ref()?;
        if completion.items.is_empty() {
            return None;
        }

        let visible_items = visible_items.max(1);
        let selected = completion.selected.min(completion.items.len() - 1);
        let start = selected.saturating_sub(visible_items.saturating_sub(1));
        let end = (start + visible_items).min(completion.items.len());
        Some((start, end, selected))
    }

    pub fn sql_cursor_line_col(&self) -> (usize, usize) {
        line_col_from_index(&self.sql.query, self.sql.cursor)
    }

    pub fn sql_cursor_screen_col(&self) -> usize {
        let (_, col) = self.sql_cursor_line_col();
        col.saturating_sub(self.sql.editor_col_offset)
    }

    pub fn sql_select_history_in_view(&mut self, index: usize) {
        let absolute = self.sql.history_offset + index;
        if absolute < self.sql.history.len() {
            self.sql.selected_history = absolute;
            self.sql.focus = SqlPane::History;
            self.ensure_sql_viewport();
        }
    }

    pub fn sql_set_cursor_from_view(&mut self, line_in_view: usize, col: usize) {
        let line = self.sql.editor_scroll + line_in_view;
        let target_col = self
            .sql
            .editor_col_offset
            .saturating_add(col.saturating_sub(1));
        self.sql.cursor = index_for_line_col(&self.sql.query, line, target_col);
        self.sql.focus = SqlPane::Editor;
        let _ = self.sql_refresh_completion();
        self.ensure_sql_viewport();
    }

    pub fn sql_select_completion_in_view(&mut self, index: usize, visible_items: usize) {
        let Some((start, end, _)) = self.sql_completion_window(visible_items) else {
            return;
        };
        let absolute = start + index;
        if absolute < end
            && let Some(completion) = &mut self.sql.completion
        {
            completion.selected = absolute;
            self.sql.focus = SqlPane::Editor;
        }
    }

    pub fn sql_apply_selected_completion(&mut self) {
        self.sql_apply_completion();
    }

    pub fn sql_focus_editor(&mut self) {
        self.sql.focus = SqlPane::Editor;
    }

    pub fn sql_focus_history(&mut self) {
        self.sql.focus = SqlPane::History;
        self.ensure_sql_viewport();
    }

    pub fn sql_focus_results(&mut self) {
        self.sql.focus = SqlPane::Results;
    }

    pub(super) fn ensure_sql_viewport(&mut self) {
        let (line, col) = self.sql_cursor_line_col();
        let max_editor_scroll = self
            .sql_query_lines()
            .len()
            .saturating_sub(self.sql.editor_height);
        self.sql.editor_scroll = self.sql.editor_scroll.min(max_editor_scroll);
        if line < self.sql.editor_scroll {
            self.sql.editor_scroll = line;
        }
        if line >= self.sql.editor_scroll + self.sql.editor_height {
            self.sql.editor_scroll = line + 1 - self.sql.editor_height;
        }

        self.sql.editor_col_offset = self.sql.editor_col_offset.min(col);
        if col < self.sql.editor_col_offset {
            self.sql.editor_col_offset = col;
        }
        if col >= self.sql.editor_col_offset + self.sql.editor_width {
            self.sql.editor_col_offset = col + 1 - self.sql.editor_width;
        }

        let max_history_offset = self
            .sql
            .history
            .len()
            .saturating_sub(self.sql.history_height);
        self.sql.history_offset = self.sql.history_offset.min(max_history_offset);
        if self.sql.selected_history < self.sql.history_offset {
            self.sql.history_offset = self.sql.selected_history;
        }
        if self.sql.selected_history >= self.sql.history_offset + self.sql.history_height {
            self.sql.history_offset = self.sql.selected_history + 1 - self.sql.history_height;
        }

        if let SqlResultState::Rows { rows, .. } = &self.sql.result {
            let max_result_scroll = rows.len().saturating_sub(self.sql.result_height);
            self.sql.result_scroll = self.sql.result_scroll.min(max_result_scroll);
        } else {
            self.sql.result_scroll = 0;
        }
    }

    pub(super) fn handle_sql(&mut self, action: Action) -> Result<()> {
        match action {
            Action::None => {}
            Action::ToggleFocus => self.sql_cycle_focus(),
            Action::ReverseFocus => self.sql_cycle_focus_back(),
            Action::MoveUp => self.sql_move_up(),
            Action::MoveDown => self.sql_move_down(),
            Action::MoveLeft => self.sql_move_left(),
            Action::MoveRight => self.sql_move_right(),
            Action::MoveHome => self.sql_move_home(),
            Action::MoveEnd => self.sql_move_end(),
            Action::PageUp => self.sql_page_up(),
            Action::PageDown => self.sql_page_down(),
            Action::InputChar(ch) => self.sql_insert_char(ch)?,
            Action::Backspace => self.sql_backspace()?,
            Action::Delete => self.sql_delete()?,
            Action::NewLine => self.sql_newline()?,
            Action::ExecuteSql => self.sql_execute()?,
            Action::Confirm => self.sql_confirm()?,
            Action::Clear => self.sql_clear(),
            Action::Reload => self.reload()?,
            Action::CloseModal => self.sql.completion = None,
            Action::Quit => {}
            Action::SwitchToBrowse
            | Action::SwitchToSql
            | Action::ToggleView
            | Action::OpenConfig
            | Action::ToggleItem
            | Action::FollowLink
            | Action::OpenSearchCurrent
            | Action::OpenSearchAll
            | Action::OpenFilters => {}
        }

        Ok(())
    }
}
