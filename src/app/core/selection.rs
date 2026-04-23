use anyhow::Result;

use super::super::App;

impl App {
    pub(in crate::app) fn move_table_selection_up(&mut self) -> Result<()> {
        if self.selected_table > 0 {
            self.selected_table -= 1;
            self.details = None;
            self.detail = None;
            self.reset_content_position();
            self.refresh_preview()?;
        }
        Ok(())
    }

    pub(in crate::app) fn move_table_selection_down(&mut self) -> Result<()> {
        if self.selected_table + 1 < self.tables.len() {
            self.selected_table += 1;
            self.details = None;
            self.detail = None;
            self.reset_content_position();
            self.refresh_preview()?;
        }
        Ok(())
    }

    pub(in crate::app) fn move_row_selection_up(&mut self) -> Result<()> {
        if self.selected_row > 0 {
            self.detail = None;
            self.selected_row -= 1;
            let previous_offset = self.row_offset;
            self.clamp_row_viewport();
            if previous_offset != self.row_offset {
                self.refresh_preview_page()?;
            }
        }
        Ok(())
    }

    pub(in crate::app) fn move_row_selection_down(&mut self) -> Result<()> {
        if self.selected_row + 1 < self.preview.total_rows {
            self.detail = None;
            self.selected_row += 1;
            let previous_offset = self.row_offset;
            self.clamp_row_viewport();
            if previous_offset != self.row_offset {
                self.refresh_preview_page()?;
            }
        }
        Ok(())
    }

    pub(in crate::app) fn scroll_schema_up(&mut self) {
        if self.schema_offset > 0 {
            self.schema_offset -= 1;
        }
    }

    pub(in crate::app) fn scroll_schema_down(&mut self) {
        let max_offset = self.max_schema_offset();
        if self.schema_offset < max_offset {
            self.schema_offset += 1;
        }
    }
}
