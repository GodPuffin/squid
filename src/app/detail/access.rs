use super::super::{App, ContentView, DetailField, DetailPane};
use super::text::{detail_value_text, wrapped_line_count};

impl App {
    pub fn detail_select_field(&mut self, index: usize) {
        let Some(detail) = &mut self.detail else {
            return;
        };
        if detail.fields.is_empty() {
            return;
        }

        detail.pane = DetailPane::Fields;
        detail.selected_field = index.min(detail.fields.len().saturating_sub(1));
        detail.value_scroll = 0;
        detail.is_editing = false;
        detail.message = None;
    }

    pub fn detail_focus_value(&mut self) {
        if let Some(detail) = &mut self.detail {
            detail.pane = DetailPane::Value;
        }
    }

    pub fn detail_scroll_value(&mut self, delta: isize) {
        if delta < 0 {
            self.detail_move_up();
        } else if delta > 0 {
            self.detail_move_down();
        }
    }

    pub fn detail_is_editing(&self) -> bool {
        self.detail.as_ref().is_some_and(|detail| detail.is_editing)
    }

    pub fn detail_has_changes(&self) -> bool {
        self.detail
            .as_ref()
            .is_some_and(|detail| detail.fields.iter().any(DetailField::is_dirty))
    }

    pub fn detail_is_new_row(&self) -> bool {
        self.detail.as_ref().is_some_and(|detail| detail.is_new_row)
    }

    pub fn can_add_new_row(&self) -> bool {
        self.content_view == ContentView::Rows
            && self.selected_table_name().is_some()
            && self.detail_database_is_writable()
    }

    pub fn detail_database_is_writable(&self) -> bool {
        self.selected_table_name()
            .and_then(|table_name| {
                self.db
                    .as_ref()
                    .and_then(|db| db.table_is_writable(table_name).ok())
            })
            .unwrap_or(false)
    }

    pub fn detail_is_row_writable(&self) -> bool {
        self.detail_database_is_writable()
            && self
                .detail
                .as_ref()
                .is_some_and(|detail| detail.is_new_row || detail.rowid.is_some())
    }

    pub fn detail_selected_field_is_editable(&self) -> bool {
        self.detail
            .as_ref()
            .and_then(|detail| detail.fields.get(detail.selected_field))
            .is_some_and(|field| !field.is_blob)
            && self.detail_is_row_writable()
    }

    pub fn detail_pane(&self) -> Option<DetailPane> {
        self.detail.as_ref().map(|detail| detail.pane)
    }

    pub(in crate::app) fn clamp_detail_scroll(&mut self) {
        let Some(detail) = &mut self.detail else {
            return;
        };
        if detail.fields.is_empty() {
            detail.selected_field = 0;
            detail.value_scroll = 0;
            return;
        }

        detail.selected_field = detail
            .selected_field
            .min(detail.fields.len().saturating_sub(1));
        let value = detail_value_text(detail, &detail.fields[detail.selected_field]);
        let line_count = wrapped_line_count(&value, detail.value_view_width);
        let max_scroll = line_count.saturating_sub(detail.value_view_height);
        detail.value_scroll = detail.value_scroll.min(max_scroll);
    }
}
