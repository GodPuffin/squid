use anyhow::Result;

use super::{App, ContentView, PaneFocus};

impl App {
    pub fn focus_tables(&mut self) {
        self.focus = PaneFocus::Tables;
    }

    pub fn focus_content(&mut self) {
        self.focus = PaneFocus::Content;
    }

    pub fn select_table_by_index(&mut self, index: usize) -> Result<()> {
        if self.is_home() {
            if index < self.recent_items.len() {
                self.selected_recent = index;
                self.focus_tables();
            }
            return Ok(());
        }

        if index < self.tables.len() && index != self.selected_table {
            self.selected_table = index;
            self.details = None;
            self.detail = None;
            self.reset_content_position();
            self.refresh_preview()?;
        } else if index < self.tables.len() {
            self.focus_tables();
        }

        Ok(())
    }

    pub(super) fn select_table_by_name(&mut self, table_name: &str) -> Result<bool> {
        let Some(index) = self
            .tables
            .iter()
            .position(|table| table.name == table_name)
        else {
            return Ok(false);
        };
        self.selected_table = index;
        self.details = None;
        self.detail = None;
        self.reset_content_position();
        self.refresh_preview()?;
        Ok(true)
    }

    pub fn select_row_in_view(&mut self, row_in_view: usize) -> Result<()> {
        self.focus_content();
        let total = self.preview.total_rows;
        if total > 0 {
            let absolute = (self.row_offset + row_in_view).min(total.saturating_sub(1));
            self.selected_row = absolute;
        }
        Ok(())
    }

    pub fn scroll_tables(&mut self, delta: isize) -> Result<()> {
        self.focus_tables();
        if self.is_home() {
            if delta < 0 {
                self.move_recent_selection_up();
            } else if delta > 0 {
                self.move_recent_selection_down();
            }
        } else if delta < 0 {
            self.move_table_selection_up()?;
        } else if delta > 0 {
            self.move_table_selection_down()?;
        }
        Ok(())
    }

    pub fn scroll_content(&mut self, delta: isize) -> Result<()> {
        self.focus_content();
        if self.is_home() {
            return Ok(());
        }
        match self.content_view {
            ContentView::Rows => {
                if delta < 0 {
                    self.move_row_selection_up()?;
                } else if delta > 0 {
                    self.move_row_selection_down()?;
                }
            }
            ContentView::Schema => {
                if delta < 0 {
                    self.scroll_schema_up();
                } else if delta > 0 {
                    self.scroll_schema_down();
                }
            }
        }
        Ok(())
    }

    pub(super) fn jump_to_row_offset(&mut self, offset: usize) -> Result<()> {
        self.selected_row = offset;
        self.row_offset = (offset / self.row_limit) * self.row_limit;
        self.content_view = ContentView::Rows;
        self.focus_content();
        self.refresh_preview()
    }
}
