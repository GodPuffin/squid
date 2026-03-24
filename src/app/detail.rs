use anyhow::Result;

use super::{Action, App, ContentView, DetailField, DetailForeignTarget, DetailPane, DetailState};

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

    pub(super) fn handle_detail(&mut self, action: Action) -> Result<()> {
        match action {
            Action::CloseModal | Action::Quit => self.detail = None,
            Action::ReverseFocus => self.detail_move_left(),
            Action::MoveLeft => self.detail_move_left(),
            Action::MoveRight | Action::ToggleFocus => self.detail_move_right(),
            Action::MoveUp => self.detail_move_up(),
            Action::MoveDown => self.detail_move_down(),
            Action::FollowLink | Action::Confirm => self.follow_detail_link()?,
            Action::None
            | Action::SwitchToBrowse
            | Action::SwitchToSql
            | Action::ToggleView
            | Action::MoveHome
            | Action::MoveEnd
            | Action::PageUp
            | Action::PageDown
            | Action::OpenConfig
            | Action::ToggleItem
            | Action::Delete
            | Action::Clear
            | Action::Reload
            | Action::OpenSearchCurrent
            | Action::OpenSearchAll
            | Action::OpenFilters
            | Action::InputChar(_)
            | Action::Backspace
            | Action::ExecuteSql
            | Action::NewLine => {}
        }

        Ok(())
    }

    pub(super) fn open_detail(&mut self) -> Result<()> {
        if self.focus != super::PaneFocus::Content || self.content_view != ContentView::Rows {
            return Ok(());
        }
        let Some(table_name) = self.selected_table_name().map(str::to_owned) else {
            return Ok(());
        };
        if self.preview.total_rows == 0 {
            return Ok(());
        }

        let record = self.db.row_record_at_offset(
            &table_name,
            &self.current_sort_clauses(),
            &self.current_filter_clauses(),
            self.selected_row,
        )?;
        let Some(record) = record else {
            return Ok(());
        };

        let fields = record
            .fields
            .into_iter()
            .map(|(column_name, value)| {
                let foreign_target = record
                    .foreign_keys
                    .iter()
                    .find(|fk| fk.from_column == column_name)
                    .and_then(|fk| {
                        if value == "NULL" {
                            None
                        } else {
                            Some(DetailForeignTarget {
                                table_name: fk.target_table.clone(),
                                column_name: fk.target_column.clone(),
                                value: value.clone(),
                            })
                        }
                    });
                DetailField {
                    column_name,
                    value,
                    foreign_target,
                }
            })
            .collect();

        self.detail = Some(DetailState {
            row_label: record.row_label,
            pane: DetailPane::Fields,
            selected_field: 0,
            value_scroll: 0,
            value_view_width: super::DEFAULT_DETAIL_VALUE_WIDTH,
            value_view_height: super::DEFAULT_DETAIL_VALUE_HEIGHT,
            fields,
        });

        Ok(())
    }

    pub(super) fn clamp_detail_scroll(&mut self) {
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
        let value = &detail.fields[detail.selected_field].value;
        let line_count = wrapped_line_count(value, detail.value_view_width);
        let max_scroll = line_count.saturating_sub(detail.value_view_height);
        detail.value_scroll = detail.value_scroll.min(max_scroll);
    }

    fn detail_move_left(&mut self) {
        if let Some(detail) = &mut self.detail {
            detail.pane = DetailPane::Fields;
        }
    }

    fn detail_move_right(&mut self) {
        if let Some(detail) = &mut self.detail {
            detail.pane = DetailPane::Value;
        }
    }

    fn detail_move_up(&mut self) {
        let Some(detail) = &mut self.detail else {
            return;
        };
        match detail.pane {
            DetailPane::Fields => {
                detail.selected_field = detail.selected_field.saturating_sub(1);
                detail.value_scroll = 0;
            }
            DetailPane::Value => {
                detail.value_scroll = detail.value_scroll.saturating_sub(1);
            }
        }
    }

    fn detail_move_down(&mut self) {
        let Some(detail) = &mut self.detail else {
            return;
        };
        match detail.pane {
            DetailPane::Fields => {
                if !detail.fields.is_empty() {
                    detail.selected_field =
                        (detail.selected_field + 1).min(detail.fields.len().saturating_sub(1));
                    detail.value_scroll = 0;
                }
            }
            DetailPane::Value => {
                detail.value_scroll = detail.value_scroll.saturating_add(1);
            }
        }
        self.clamp_detail_scroll();
    }

    fn follow_detail_link(&mut self) -> Result<()> {
        let target = self
            .detail
            .as_ref()
            .and_then(|detail| detail.fields.get(detail.selected_field))
            .and_then(|field| field.foreign_target.clone());
        let Some(target) = target else {
            return Ok(());
        };

        if !self.select_table_by_name(&target.table_name)? {
            return Ok(());
        }

        self.detail = None;
        let Some(offset) = self.db.locate_foreign_row_offset(
            &target.table_name,
            &target.column_name,
            &target.value,
            &self.current_sort_clauses(),
            &self.current_filter_clauses(),
        )?
        else {
            return Ok(());
        };

        self.jump_to_row_offset(offset)
    }
}

fn wrapped_line_count(value: &str, width: usize) -> usize {
    let width = width.max(1);
    let mut count = 0;

    for line in value.lines() {
        let chars = line.chars().count();
        count += chars.max(1).div_ceil(width);
    }

    if count == 0 { 1 } else { count }
}
