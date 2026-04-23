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
            self.refresh_preview()?;
        }

        Ok(())
    }

    pub(in crate::app) fn refresh_preview(&mut self) -> Result<()> {
        if self.is_home() {
            return Ok(());
        }

        if let Some(table_name) = self.selected_table_name().map(str::to_owned) {
            let db = self.db_ref()?;
            self.details = Some(db.table_details(&table_name)?);
            self.ensure_table_config();

            let queried_offset = self.row_offset;
            self.preview = self.db_ref()?.preview_table(
                &table_name,
                &self.visible_column_names(),
                &self.current_sort_clauses(),
                &self.current_filter_clauses(),
                self.row_limit,
                self.row_offset,
            )?;
            if let Some(details) = &mut self.details {
                details.total_rows = self.preview.total_rows;
            }
            self.clamp_row_viewport();
            if self.row_offset != queried_offset {
                self.preview = self.db_ref()?.preview_table(
                    &table_name,
                    &self.visible_column_names(),
                    &self.current_sort_clauses(),
                    &self.current_filter_clauses(),
                    self.row_limit,
                    self.row_offset,
                )?;
            }
            self.clamp_schema_offset();
        } else {
            self.details = None;
            self.preview = RowPreview::empty();
            self.selected_row = 0;
            self.row_offset = 0;
            self.schema_offset = 0;
            self.modal = None;
            self.close_search();
            self.detail = None;
        }

        Ok(())
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
