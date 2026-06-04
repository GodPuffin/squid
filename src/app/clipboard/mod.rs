use anyhow::Result;

use crate::runtime::clipboard;

use super::{
    App, AppMode, ContentView, DetailMessage, PaneFocus, SqlPane, detail::detail_value_text,
};

impl App {
    pub(in crate::app) fn copy_to_clipboard(&mut self) -> Result<()> {
        let Some(text) = self.copy_text() else {
            return Ok(());
        };

        match clipboard::set_clipboard(&text) {
            Ok(()) => self.set_copy_feedback("Copied to clipboard"),
            Err(error) => self.set_copy_feedback(format!("Copy failed: {error}")),
        }

        Ok(())
    }

    fn copy_text(&self) -> Option<String> {
        if self.detail.is_some() {
            return self.detail_copy_text();
        }

        if self.mode == AppMode::Sql {
            return self.sql_copy_text();
        }

        if self.focus == PaneFocus::Content && self.content_view == ContentView::Rows {
            return self.preview_cell_copy_text();
        }

        None
    }

    pub(crate) fn preview_cell_copy_text(&self) -> Option<String> {
        if self.preview.columns.is_empty() || self.preview.total_rows == 0 {
            return None;
        }

        let row_in_view = self.selected_row_in_view()?;
        let row = self.preview.rows.get(row_in_view)?;
        let column = self.selected_column.min(row.len().saturating_sub(1));
        row.get(column).cloned()
    }

    pub(crate) fn detail_copy_text(&self) -> Option<String> {
        let detail = self.detail.as_ref()?;
        let field = detail.fields.get(detail.selected_field)?;
        Some(detail_value_text(detail, field))
    }

    pub(crate) fn sql_copy_text(&self) -> Option<String> {
        if self.sql_focus() != SqlPane::Editor || self.sql.query.is_empty() {
            return None;
        }
        Some(self.sql.query.clone())
    }

    fn set_copy_feedback(&mut self, message: impl Into<String>) {
        let message = message.into();
        if self.detail.is_some() {
            if let Some(detail) = &mut self.detail {
                detail.message = Some(DetailMessage {
                    text: message,
                    is_error: false,
                });
            }
            return;
        }

        if self.mode == AppMode::Sql {
            self.sql.status = message;
            return;
        }

        self.status_message = Some(message);
    }

    pub(in crate::app) fn move_preview_column_left(&mut self) {
        if self.selected_column > 0 {
            self.selected_column -= 1;
        }
    }

    pub(in crate::app) fn move_preview_column_right(&mut self) {
        let max_column = self.preview.columns.len().saturating_sub(1);
        if self.selected_column < max_column {
            self.selected_column += 1;
        }
    }
}

#[cfg(test)]
mod tests;
