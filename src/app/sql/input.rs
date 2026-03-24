use anyhow::Result;

use super::cursor::{
    index_for_line_col, line_col_from_index, line_length, move_vertical, next_boundary,
    previous_boundary,
};
use super::{App, SqlPane, SqlResultState};

impl App {
    pub(super) fn sql_cycle_focus(&mut self) {
        self.sql.focus = match self.sql.focus {
            SqlPane::Editor => SqlPane::History,
            SqlPane::History => SqlPane::Results,
            SqlPane::Results => SqlPane::Editor,
        };
        self.sql.completion = None;
    }

    pub(super) fn sql_cycle_focus_back(&mut self) {
        self.sql.focus = match self.sql.focus {
            SqlPane::Editor => SqlPane::Results,
            SqlPane::History => SqlPane::Editor,
            SqlPane::Results => SqlPane::History,
        };
        self.sql.completion = None;
    }

    pub(super) fn sql_move_up(&mut self) {
        if self.sql.focus == SqlPane::Editor
            && let Some(completion) = &mut self.sql.completion
            && completion.selected > 0
        {
            completion.selected -= 1;
            return;
        }

        match self.sql.focus {
            SqlPane::Editor => {
                self.sql.cursor = move_vertical(&self.sql.query, self.sql.cursor, -1);
                let _ = self.sql_refresh_completion();
                self.ensure_sql_viewport();
            }
            SqlPane::History => {
                if self.sql.selected_history > 0 {
                    self.sql.selected_history -= 1;
                    self.ensure_sql_viewport();
                }
            }
            SqlPane::Results => {
                if self.sql.result_scroll > 0 {
                    self.sql.result_scroll -= 1;
                }
            }
        }
    }

    pub(super) fn sql_move_down(&mut self) {
        if self.sql.focus == SqlPane::Editor
            && let Some(completion) = &mut self.sql.completion
            && completion.selected + 1 < completion.items.len()
        {
            completion.selected += 1;
            return;
        }

        match self.sql.focus {
            SqlPane::Editor => {
                self.sql.cursor = move_vertical(&self.sql.query, self.sql.cursor, 1);
                let _ = self.sql_refresh_completion();
                self.ensure_sql_viewport();
            }
            SqlPane::History => {
                if self.sql.selected_history + 1 < self.sql.history.len() {
                    self.sql.selected_history += 1;
                    self.ensure_sql_viewport();
                }
            }
            SqlPane::Results => {
                if let SqlResultState::Rows { rows, .. } = &self.sql.result
                    && self.sql.result_scroll + self.sql.result_height < rows.len()
                {
                    self.sql.result_scroll += 1;
                }
            }
        }
    }

    pub(super) fn sql_move_left(&mut self) {
        if self.sql.focus == SqlPane::Editor {
            self.sql.cursor = previous_boundary(&self.sql.query, self.sql.cursor);
            let _ = self.sql_refresh_completion();
            self.ensure_sql_viewport();
        }
    }

    pub(super) fn sql_move_right(&mut self) {
        if self.sql.focus == SqlPane::Editor {
            self.sql.cursor = next_boundary(&self.sql.query, self.sql.cursor);
            let _ = self.sql_refresh_completion();
            self.ensure_sql_viewport();
        }
    }

    pub(super) fn sql_move_home(&mut self) {
        match self.sql.focus {
            SqlPane::Editor => {
                let (line, _) = line_col_from_index(&self.sql.query, self.sql.cursor);
                self.sql.cursor = index_for_line_col(&self.sql.query, line, 0);
                let _ = self.sql_refresh_completion();
                self.ensure_sql_viewport();
            }
            SqlPane::History => {
                self.sql.selected_history = 0;
                self.ensure_sql_viewport();
            }
            SqlPane::Results => {
                self.sql.result_scroll = 0;
            }
        }
    }

    pub(super) fn sql_move_end(&mut self) {
        match self.sql.focus {
            SqlPane::Editor => {
                let (line, _) = line_col_from_index(&self.sql.query, self.sql.cursor);
                let len = line_length(&self.sql.query, line);
                self.sql.cursor = index_for_line_col(&self.sql.query, line, len);
                let _ = self.sql_refresh_completion();
                self.ensure_sql_viewport();
            }
            SqlPane::History => {
                if !self.sql.history.is_empty() {
                    self.sql.selected_history = self.sql.history.len() - 1;
                    self.ensure_sql_viewport();
                }
            }
            SqlPane::Results => {
                if let SqlResultState::Rows { rows, .. } = &self.sql.result {
                    self.sql.result_scroll = rows.len().saturating_sub(self.sql.result_height);
                }
            }
        }
    }

    pub(super) fn sql_page_up(&mut self) {
        match self.sql.focus {
            SqlPane::Editor => {
                for _ in 0..self.sql.editor_height {
                    self.sql.cursor = move_vertical(&self.sql.query, self.sql.cursor, -1);
                }
                let _ = self.sql_refresh_completion();
                self.ensure_sql_viewport();
            }
            SqlPane::History => {
                self.sql.selected_history = self
                    .sql
                    .selected_history
                    .saturating_sub(self.sql.history_height);
                self.ensure_sql_viewport();
            }
            SqlPane::Results => {
                self.sql.result_scroll = self
                    .sql
                    .result_scroll
                    .saturating_sub(self.sql.result_height);
            }
        }
    }

    pub(super) fn sql_page_down(&mut self) {
        match self.sql.focus {
            SqlPane::Editor => {
                for _ in 0..self.sql.editor_height {
                    self.sql.cursor = move_vertical(&self.sql.query, self.sql.cursor, 1);
                }
                let _ = self.sql_refresh_completion();
                self.ensure_sql_viewport();
            }
            SqlPane::History => {
                if !self.sql.history.is_empty() {
                    self.sql.selected_history = (self.sql.selected_history
                        + self.sql.history_height)
                        .min(self.sql.history.len() - 1);
                    self.ensure_sql_viewport();
                }
            }
            SqlPane::Results => {
                if let SqlResultState::Rows { rows, .. } = &self.sql.result {
                    self.sql.result_scroll = (self.sql.result_scroll + self.sql.result_height)
                        .min(rows.len().saturating_sub(self.sql.result_height));
                }
            }
        }
    }

    pub(super) fn sql_insert_char(&mut self, ch: char) -> Result<()> {
        if self.sql.focus != SqlPane::Editor {
            return Ok(());
        }
        self.sql.query.insert(self.sql.cursor, ch);
        self.sql.cursor += ch.len_utf8();
        self.sql_refresh_completion()?;
        self.ensure_sql_viewport();
        Ok(())
    }

    pub(super) fn sql_backspace(&mut self) -> Result<()> {
        if self.sql.focus != SqlPane::Editor || self.sql.cursor == 0 {
            return Ok(());
        }
        let start = previous_boundary(&self.sql.query, self.sql.cursor);
        self.sql.query.replace_range(start..self.sql.cursor, "");
        self.sql.cursor = start;
        self.sql_refresh_completion()?;
        self.ensure_sql_viewport();
        Ok(())
    }

    pub(super) fn sql_delete(&mut self) -> Result<()> {
        if self.sql.focus != SqlPane::Editor || self.sql.cursor >= self.sql.query.len() {
            return Ok(());
        }
        let end = next_boundary(&self.sql.query, self.sql.cursor);
        self.sql.query.replace_range(self.sql.cursor..end, "");
        self.sql_refresh_completion()?;
        self.ensure_sql_viewport();
        Ok(())
    }

    pub(super) fn sql_newline(&mut self) -> Result<()> {
        match self.sql.focus {
            SqlPane::Editor => {
                if self.sql.completion.is_some() {
                    self.sql_apply_completion();
                    return Ok(());
                }
                self.sql_insert_char('\n')?;
            }
            SqlPane::History => self.sql_load_history_selected(),
            SqlPane::Results => {}
        }
        Ok(())
    }

    pub(super) fn sql_confirm(&mut self) -> Result<()> {
        match self.sql.focus {
            SqlPane::Editor => {
                if self.sql.completion.is_some() {
                    self.sql_apply_completion();
                } else {
                    self.sql_refresh_completion()?;
                }
            }
            SqlPane::History => self.sql_load_history_selected(),
            SqlPane::Results => {}
        }
        Ok(())
    }

    pub(super) fn sql_clear(&mut self) {
        match self.sql.focus {
            SqlPane::Editor => {
                self.sql.query.clear();
                self.sql.cursor = 0;
                self.sql.editor_scroll = 0;
                self.sql.completion = None;
                self.sql.status = "Query cleared".to_string();
            }
            SqlPane::History => {
                self.sql.history.clear();
                self.sql.history_offset = 0;
                self.sql.selected_history = 0;
                self.sql.status = "History cleared".to_string();
            }
            SqlPane::Results => {
                self.sql.result = SqlResultState::Empty;
                self.sql.result_scroll = 0;
                self.sql.status = "Results cleared".to_string();
            }
        }
    }
}
