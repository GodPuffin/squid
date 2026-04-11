use anyhow::Result;

use super::{Action, App, SearchScope, SearchState};
use crate::db::SearchHit;

const CURRENT_TABLE_LIVE_SEARCH_MAX_ROWS: usize = 2_000;
const HORIZONTAL_SEARCH_SCROLL_STEP: usize = 8;
const SEARCH_RESULTS_BLOCK_BORDER_WIDTH: usize = 2;
const SEARCH_RESULTS_HIGHLIGHT_SYMBOL_WIDTH: usize = 3;

impl App {
    pub fn close_search(&mut self) {
        self.search = None;
    }

    pub fn select_search_result_in_view(&mut self, index: usize) {
        if let Some(search) = &mut self.search {
            let absolute = search.result_offset + index;
            if absolute < search.results.len() {
                search.selected_result = absolute;
                self.clamp_search_viewport();
            }
        }
    }

    pub fn scroll_search(&mut self, delta: isize) {
        if delta < 0 {
            let _ = self.handle_search(Action::MoveUp);
        } else if delta > 0 {
            let _ = self.handle_search(Action::MoveDown);
        }
    }

    pub(crate) fn sync_search_results_view_width(&mut self, area_width: usize) {
        self.search_results_view_width = area_width
            .saturating_sub(SEARCH_RESULTS_BLOCK_BORDER_WIDTH)
            .saturating_sub(SEARCH_RESULTS_HIGHLIGHT_SYMBOL_WIDTH);
        self.clamp_search_viewport();
    }

    pub(super) fn handle_search(&mut self, action: Action) -> Result<()> {
        match action {
            Action::CloseModal => self.close_search(),
            Action::MoveUp => self.search_move_up(),
            Action::MoveDown => self.search_move_down(),
            Action::MoveLeft => self.search_move_left(),
            Action::MoveRight => self.search_move_right(),
            Action::Confirm => self.search_confirm()?,
            Action::InputChar(ch) => {
                if let Some(search) = &mut self.search {
                    search.query.push(ch);
                }
                self.refresh_search_if_live()?;
            }
            Action::Backspace => {
                if let Some(search) = &mut self.search {
                    search.query.pop();
                }
                self.refresh_search_if_live()?;
            }
            Action::OpenSearchCurrent => self.open_search(SearchScope::CurrentTable)?,
            Action::OpenSearchAll => self.open_search(SearchScope::AllTables)?,
            Action::Quit => {}
            Action::None
            | Action::SwitchToBrowse
            | Action::SwitchToSql
            | Action::ToggleFocus
            | Action::ReverseFocus
            | Action::ToggleView
            | Action::MoveHome
            | Action::MoveEnd
            | Action::PageUp
            | Action::PageDown
            | Action::OpenConfig
            | Action::ToggleItem
            | Action::Delete
            | Action::Clear
            | Action::FollowLink
            | Action::EditDetail
            | Action::SaveDetail
            | Action::DiscardDetail
            | Action::OpenFilters
            | Action::Reload
            | Action::ExecuteSql
            | Action::NewLine => {}
        }

        Ok(())
    }

    pub(super) fn open_search(&mut self, scope: SearchScope) -> Result<()> {
        self.focus_content();
        let submitted =
            matches!(scope, SearchScope::CurrentTable) && self.current_table_search_is_live();
        self.search = Some(SearchState {
            scope,
            query: String::new(),
            results: Vec::new(),
            selected_result: 0,
            result_offset: 0,
            horizontal_offset: 0,
            result_limit: self.row_limit.saturating_sub(3).max(1),
            submitted,
            loading: false,
        });
        if submitted {
            self.refresh_search()?;
        }
        Ok(())
    }

    fn refresh_search_if_live(&mut self) -> Result<()> {
        if self.search.as_ref().is_some_and(|search| {
            matches!(search.scope, SearchScope::CurrentTable) && self.current_table_search_is_live()
        }) {
            self.refresh_search()?;
        } else if let Some(search) = &mut self.search {
            search.submitted = false;
            search.loading = false;
            search.results.clear();
            search.selected_result = 0;
            search.result_offset = 0;
            search.horizontal_offset = 0;
        }

        Ok(())
    }

    fn refresh_search(&mut self) -> Result<()> {
        let Some(search) = &self.search else {
            return Ok(());
        };

        let scope = search.scope;
        let query = search.query.clone();
        let visible_columns = self.visible_column_names();
        let filter_clauses = self.current_filter_clauses();
        let current_table = self.selected_table_name().map(str::to_owned);

        let results = match scope {
            SearchScope::CurrentTable => {
                if let Some(table_name) = current_table {
                    self.db_ref()?.search_table(
                        &table_name,
                        &visible_columns,
                        &self.current_sort_clauses(),
                        &filter_clauses,
                        &query,
                        200,
                    )?
                } else {
                    Vec::new()
                }
            }
            SearchScope::AllTables => self.db_ref()?.search_tables(&self.tables, &query, 300)?,
        };
        if let Some(search) = &mut self.search {
            search.results = results;
            search.submitted = true;
            search.loading = false;
            if search.results.is_empty() {
                search.selected_result = 0;
                search.result_offset = 0;
                search.horizontal_offset = 0;
            } else {
                search.selected_result = search
                    .selected_result
                    .min(search.results.len().saturating_sub(1));
                self.clamp_search_viewport();
            }
        }

        Ok(())
    }

    fn search_confirm(&mut self) -> Result<()> {
        let Some(search) = &self.search else {
            return Ok(());
        };

        match search.scope {
            SearchScope::CurrentTable => {
                if search.submitted {
                    self.jump_to_search_result()?;
                } else {
                    self.schedule_search_refresh();
                }
            }
            SearchScope::AllTables => {
                if search.submitted {
                    self.jump_to_search_result()?;
                } else {
                    self.schedule_search_refresh();
                }
            }
        }

        Ok(())
    }

    fn search_move_up(&mut self) {
        if let Some(search) = &mut self.search
            && search.selected_result > 0
        {
            search.selected_result -= 1;
            self.clamp_search_viewport();
        }
    }

    fn search_move_down(&mut self) {
        if let Some(search) = &mut self.search
            && search.selected_result + 1 < search.results.len()
        {
            search.selected_result += 1;
            self.clamp_search_viewport();
        }
    }

    fn search_move_left(&mut self) {
        if let Some(search) = &mut self.search
            && matches!(search.scope, SearchScope::AllTables)
        {
            search.horizontal_offset = search
                .horizontal_offset
                .saturating_sub(HORIZONTAL_SEARCH_SCROLL_STEP);
        }
    }

    fn search_move_right(&mut self) {
        let max_offset = self.max_all_table_search_horizontal_offset();
        if let Some(search) = &mut self.search
            && matches!(search.scope, SearchScope::AllTables)
        {
            search.horizontal_offset =
                (search.horizontal_offset + HORIZONTAL_SEARCH_SCROLL_STEP).min(max_offset);
        }
    }

    pub(super) fn clamp_search_viewport(&mut self) {
        let max_horizontal_offset = self
            .search
            .as_ref()
            .map(|search| {
                self.max_all_table_search_horizontal_offset_for_results(
                    search.scope,
                    &search.results,
                )
            })
            .unwrap_or(0);
        let Some(search) = &mut self.search else {
            return;
        };

        search.horizontal_offset = search.horizontal_offset.min(max_horizontal_offset);
        let max_offset = search.results.len().saturating_sub(search.result_limit);
        search.result_offset = search.result_offset.min(max_offset);

        if search.selected_result < search.result_offset {
            search.result_offset = search.selected_result;
        }
        if search.selected_result >= search.result_offset + search.result_limit {
            search.result_offset = search.selected_result + 1 - search.result_limit;
        }
    }

    fn max_all_table_search_horizontal_offset(&self) -> usize {
        let Some(search) = &self.search else {
            return 0;
        };

        self.max_all_table_search_horizontal_offset_for_results(search.scope, &search.results)
    }

    fn max_all_table_search_horizontal_offset_for_results(
        &self,
        scope: SearchScope,
        results: &[SearchHit],
    ) -> usize {
        if !matches!(scope, SearchScope::AllTables) || self.search_results_view_width == 0 {
            return 0;
        }

        results
            .iter()
            .map(|hit| {
                self.all_table_search_result_width(hit)
                    .saturating_sub(self.search_results_view_width)
            })
            .max()
            .unwrap_or(0)
    }

    fn all_table_search_result_width(&self, hit: &SearchHit) -> usize {
        format!(
            "{}  {}  {}",
            self.display_table_name(&hit.table_name),
            hit.row_label,
            hit.haystack
        )
        .chars()
        .count()
    }

    fn jump_to_search_result(&mut self) -> Result<()> {
        let Some(search) = &self.search else {
            return Ok(());
        };
        let Some(hit) = search.results.get(search.selected_result).cloned() else {
            return Ok(());
        };
        let scope = search.scope;

        if !self.select_table_by_name(&hit.table_name)? {
            return Ok(());
        }

        let sort_clauses = self.current_sort_clauses();
        let filter_clauses = self.current_filter_clauses();
        let offset = if let Some(rowid) = hit.rowid {
            self.db_ref()?.locate_row_offset(
                &hit.table_name,
                rowid,
                &sort_clauses,
                &filter_clauses,
            )?
        } else {
            match scope {
                SearchScope::CurrentTable => Some(hit.row_offset),
                SearchScope::AllTables if sort_clauses.is_empty() && filter_clauses.is_empty() => {
                    Some(hit.row_offset)
                }
                SearchScope::AllTables => {
                    self.status_message = Some(
                        "Result found, but rowid is unavailable for this filtered/sorted table view"
                            .to_string(),
                    );
                    None
                }
            }
        };

        if let Some(offset) = offset {
            self.search = None;
            self.jump_to_row_offset(offset)?;
        }

        Ok(())
    }

    fn current_table_search_is_live(&self) -> bool {
        self.preview.total_rows <= CURRENT_TABLE_LIVE_SEARCH_MAX_ROWS
    }

    pub(crate) fn run_pending_work(&mut self) -> Result<bool> {
        if self.search.as_ref().is_some_and(|search| search.loading) {
            self.refresh_search()?;
            return Ok(true);
        }

        Ok(false)
    }

    fn schedule_search_refresh(&mut self) {
        if let Some(search) = &mut self.search {
            search.loading = true;
            search.submitted = false;
            search.results.clear();
            search.selected_result = 0;
            search.result_offset = 0;
            search.horizontal_offset = 0;
        }
    }
}

#[cfg(test)]
mod tests;
