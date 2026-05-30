use anyhow::Result;

use crate::db::SqlExecutionResult;

use super::{App, SqlHistoryEntry, SqlPane, SqlResultState};

impl App {
    pub(super) fn sql_result_row_limit(&self) -> usize {
        self.app_settings.sql_result_row_limit.max(1)
    }

    pub(super) fn sql_execute(&mut self) -> Result<()> {
        let query = self.sql.query.trim().to_string();
        if query.is_empty() {
            self.sql.result = SqlResultState::Message {
                text: "Query is empty".to_string(),
                is_error: true,
            };
            self.sql.status = "Execution failed".to_string();
            return Ok(());
        }

        match self
            .db_ref()?
            .execute_sql(&query, self.sql_result_row_limit())
        {
            Ok(SqlExecutionResult::Rows {
                columns,
                rows,
                is_mutation,
                is_truncated,
            }) => {
                let row_count = rows.len();
                let summary =
                    sql_rows_summary(row_count, is_truncated, self.sql_result_row_limit());
                self.sql.result = SqlResultState::Rows { columns, rows };
                self.sql.result_scroll = 0;
                self.sql.status = summary.clone();
                self.push_sql_history(query, summary);
                if is_mutation {
                    self.refresh_loaded_db_state()?;
                }
            }
            Ok(SqlExecutionResult::Statement {
                affected_rows,
                description,
            }) => {
                let text = format!("{description} ok ({affected_rows} row(s) affected)");
                self.sql.result = SqlResultState::Message {
                    text: text.clone(),
                    is_error: false,
                };
                self.sql.result_scroll = 0;
                self.sql.status = text.clone();
                self.push_sql_history(query, text);
                self.refresh_loaded_db_state()?;
            }
            Err(err) => {
                let text = err.to_string();
                self.sql.result = SqlResultState::Message {
                    text: text.clone(),
                    is_error: true,
                };
                self.sql.status = "Execution failed".to_string();
                self.push_sql_history(query, format!("Error: {text}"));
            }
        }

        self.sql.focus = SqlPane::Results;
        self.sql.completion = None;
        self.ensure_sql_viewport();
        Ok(())
    }

    pub(super) fn push_sql_history(&mut self, query: String, summary: String) {
        if self
            .sql
            .history
            .last()
            .is_some_and(|entry| entry.query == query)
        {
            if let Some(last) = self.sql.history.last_mut() {
                last.summary = summary;
            }
        } else {
            self.sql.history.push(SqlHistoryEntry {
                query: query.clone(),
                summary,
            });
        }
        self.trim_sql_history();
        if !self.sql.history.is_empty() {
            self.sql.selected_history = self.sql.history.len() - 1;
        }
        self.ensure_sql_viewport();
    }

    pub(in crate::app) fn trim_sql_history(&mut self) {
        let max = self.app_settings.sql_history_size;
        if self.sql.history.len() <= max {
            return;
        }

        let remove = self.sql.history.len() - max;
        self.sql.history.drain(0..remove);
        self.sql.selected_history = self
            .sql
            .selected_history
            .saturating_sub(remove)
            .min(self.sql.history.len().saturating_sub(1));
    }

    pub(super) fn sql_load_history_selected(&mut self) {
        if let Some(entry) = self.sql.history.get(self.sql.selected_history) {
            self.sql.query = entry.query.clone();
            self.sql.cursor = self.sql.query.len();
            self.sql.focus = SqlPane::Editor;
            self.sql.completion = None;
            self.ensure_sql_viewport();
        }
    }
}

pub(super) fn sql_rows_summary(row_count: usize, is_truncated: bool, limit: usize) -> String {
    if is_truncated {
        format!("Returned {row_count} row(s) (truncated at {limit})")
    } else {
        format!("Returned {row_count} row(s)")
    }
}

#[cfg(test)]
mod tests;
