use anyhow::Result;

use super::{Action, App, SearchScope, SearchState};

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

    pub(super) fn handle_search(&mut self, action: Action) -> Result<()> {
        match action {
            Action::CloseModal => self.close_search(),
            Action::MoveUp => self.search_move_up(),
            Action::MoveDown => self.search_move_down(),
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
            | Action::ToggleFocus
            | Action::ToggleView
            | Action::MoveLeft
            | Action::MoveRight
            | Action::OpenConfig
            | Action::ToggleItem
            | Action::Delete
            | Action::Clear
            | Action::FollowLink
            | Action::OpenFilters
            | Action::Reload => {}
        }

        Ok(())
    }

    pub(super) fn open_search(&mut self, scope: SearchScope) -> Result<()> {
        self.focus_content();
        self.search = Some(SearchState {
            scope,
            query: String::new(),
            results: Vec::new(),
            selected_result: 0,
            result_offset: 0,
            result_limit: self.row_limit.saturating_sub(3).max(1),
            submitted: matches!(scope, SearchScope::CurrentTable),
        });
        if matches!(scope, SearchScope::CurrentTable) {
            self.refresh_search()?;
        }
        Ok(())
    }

    fn refresh_search_if_live(&mut self) -> Result<()> {
        if self
            .search
            .as_ref()
            .is_some_and(|search| matches!(search.scope, SearchScope::CurrentTable))
        {
            self.refresh_search()?;
        } else if let Some(search) = &mut self.search {
            search.submitted = false;
            search.results.clear();
            search.selected_result = 0;
            search.result_offset = 0;
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
            if search.results.is_empty() {
                search.selected_result = 0;
                search.result_offset = 0;
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
            SearchScope::CurrentTable => self.jump_to_search_result()?,
            SearchScope::AllTables => {
                if search.submitted {
                    self.jump_to_search_result()?;
                } else {
                    self.refresh_search()?;
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

    pub(super) fn clamp_search_viewport(&mut self) {
        let Some(search) = &mut self.search else {
            return;
        };

        let max_offset = search.results.len().saturating_sub(search.result_limit);
        search.result_offset = search.result_offset.min(max_offset);

        if search.selected_result < search.result_offset {
            search.result_offset = search.selected_result;
        }
        if search.selected_result >= search.result_offset + search.result_limit {
            search.result_offset = search.selected_result + 1 - search.result_limit;
        }
    }

    fn jump_to_search_result(&mut self) -> Result<()> {
        let Some(search) = &self.search else {
            return Ok(());
        };
        let Some(hit) = search.results.get(search.selected_result).cloned() else {
            return Ok(());
        };
        let Some(rowid) = hit.rowid else {
            return Ok(());
        };

        if !self.select_table_by_name(&hit.table_name)? {
            return Ok(());
        }

        if let Some(offset) = self.db_ref()?.locate_row_offset(
            &hit.table_name,
            rowid,
            &self.current_sort_clauses(),
            &self.current_filter_clauses(),
        )? {
            self.search = None;
            self.jump_to_row_offset(offset)?;
        }

        Ok(())
    }
}
