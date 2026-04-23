use anyhow::Result;

use crate::db::RowPreview;

use super::super::App;

impl App {
    pub fn set_viewport_sizes(
        &mut self,
        row_limit: usize,
        schema_page_lines: usize,
        detail_value_width: usize,
        detail_value_height: usize,
    ) -> Result<()> {
        if self.is_home() {
            return Ok(());
        }

        let row_limit = row_limit.max(1);
        let schema_page_lines = schema_page_lines.max(1);
        let detail_value_width = detail_value_width.max(1);
        let detail_value_height = detail_value_height.max(1);
        let mut needs_refresh = false;

        if self.row_limit != row_limit {
            self.row_limit = row_limit;
            self.clamp_row_viewport();
            needs_refresh = true;
        }

        if self.schema_page_lines != schema_page_lines {
            self.schema_page_lines = schema_page_lines;
            self.clamp_schema_offset();
        }

        if let Some(detail) = &mut self.detail {
            detail.value_view_width = detail_value_width;
            detail.value_view_height = detail_value_height;
            self.clamp_detail_scroll();
        }

        if let Some(search) = &mut self.search {
            search.result_limit = row_limit.saturating_sub(3).max(1);
            self.clamp_search_viewport();
        }

        self.ensure_sql_viewport();

        if needs_refresh {
            self.refresh_preview_page()?;
        }

        Ok(())
    }

    pub(in crate::app) fn refresh_preview(&mut self) -> Result<()> {
        self.details = None;
        self.refresh_preview_inner(true)
    }

    pub(in crate::app) fn refresh_preview_page(&mut self) -> Result<()> {
        if self.details.is_none() {
            return self.refresh_preview();
        }

        self.refresh_preview_inner(false)
    }

    fn refresh_preview_inner(&mut self, refresh_total_rows: bool) -> Result<()> {
        if self.is_home() {
            return Ok(());
        }

        if let Some(table_name) = self.selected_table_name().map(str::to_owned) {
            self.ensure_selected_table_details(&table_name)?;

            let visible_columns = self.visible_column_names();
            let sort_clauses = self.current_sort_clauses();
            let filter_clauses = self.current_filter_clauses();
            let total_rows = if refresh_total_rows {
                self.db_ref()?
                    .count_table_rows(&table_name, &filter_clauses)?
            } else {
                self.details
                    .as_ref()
                    .map(|details| details.total_rows)
                    .unwrap_or(self.preview.total_rows)
            };

            if let Some(details) = &mut self.details {
                details.total_rows = total_rows;
            }
            self.preview.total_rows = total_rows;
            self.clamp_row_viewport();
            self.preview = self.db_ref()?.preview_table_page(
                &table_name,
                &visible_columns,
                &sort_clauses,
                &filter_clauses,
                self.row_limit,
                self.row_offset,
                total_rows,
            )?;
            let expected_rows = total_rows
                .saturating_sub(self.row_offset)
                .min(self.row_limit);
            if !refresh_total_rows && total_rows > 0 && self.preview.rows.len() < expected_rows {
                return self.refresh_preview();
            }
            self.clamp_schema_offset();
        } else {
            self.clear_preview_state();
        }

        Ok(())
    }

    fn ensure_selected_table_details(&mut self, table_name: &str) -> Result<()> {
        if self.details.is_none() {
            self.details = Some(self.db_ref()?.table_details(table_name)?);
        }
        self.ensure_table_config();
        Ok(())
    }

    fn clear_preview_state(&mut self) {
        self.details = None;
        self.preview = RowPreview::empty();
        self.selected_row = 0;
        self.row_offset = 0;
        self.schema_offset = 0;
        self.modal = None;
        self.search = None;
        self.detail = None;
    }

    pub(super) fn clamp_row_viewport(&mut self) {
        let total_rows = self.preview.total_rows;
        if total_rows == 0 {
            self.selected_row = 0;
            self.row_offset = 0;
            return;
        }

        self.selected_row = self.selected_row.min(total_rows.saturating_sub(1));
        let max_offset = total_rows.saturating_sub(self.row_limit);
        self.row_offset = self.row_offset.min(max_offset);

        if self.selected_row < self.row_offset {
            self.row_offset = self.selected_row;
        }
        if self.selected_row >= self.row_offset + self.row_limit {
            self.row_offset = self.selected_row + 1 - self.row_limit;
        }
    }

    pub(super) fn clamp_schema_offset(&mut self) {
        self.schema_offset = self.schema_offset.min(self.max_schema_offset());
    }

    pub(super) fn max_schema_offset(&self) -> usize {
        self.schema_lines()
            .len()
            .saturating_sub(self.schema_page_lines)
    }

    pub(in crate::app) fn reset_content_position(&mut self) {
        self.selected_row = 0;
        self.row_offset = 0;
        self.schema_offset = 0;
    }
}
