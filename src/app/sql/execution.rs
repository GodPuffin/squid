use anyhow::Result;

use crate::db::SqlExecutionResult;

use super::{App, SqlHistoryEntry, SqlPane, SqlResultState};

pub(super) const SQL_RESULT_LIMIT: usize = 200;

impl App {
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

        match self.db_ref()?.execute_sql(&query, SQL_RESULT_LIMIT) {
            Ok(SqlExecutionResult::Rows {
                columns,
                rows,
                is_mutation,
                is_truncated,
            }) => {
                let row_count = rows.len();
                let summary = sql_rows_summary(row_count, is_truncated);
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
        if !self.sql.history.is_empty() {
            self.sql.selected_history = self.sql.history.len() - 1;
        }
        self.ensure_sql_viewport();
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

pub(super) fn sql_rows_summary(row_count: usize, is_truncated: bool) -> String {
    if is_truncated {
        format!("Returned {row_count} row(s) (truncated at {SQL_RESULT_LIMIT})")
    } else {
        format!("Returned {row_count} row(s)")
    }
}

#[cfg(test)]
mod tests;
