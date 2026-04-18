use std::collections::HashMap;

use anyhow::Result;

use super::super::{
    App, FilterRule, SortRule, SqlResultState, TableConfig,
    home::{AppStorage, StoredFilterRule, StoredSession, StoredSortRule, StoredTableState},
};

impl App {
    pub(super) fn restore_session_state(&mut self, session: Option<StoredSession>) -> Result<()> {
        let Some(session) = session else {
            self.refresh_preview()?;
            return Ok(());
        };

        self.mode = match session.mode {
            super::super::AppMode::Home => super::super::AppMode::Browse,
            mode => mode,
        };
        self.focus = session.focus;
        self.content_view = session.content_view;
        self.sql.query = session.sql_query;
        self.sql.cursor = session.sql_cursor.min(self.sql.query.len());
        self.sql.focus = session.sql_focus;
        self.sql.history = session.sql_history;
        self.sql.selected_history = self.sql.history.len().saturating_sub(1);
        self.sql.result = SqlResultState::Empty;
        self.sql.result_scroll = 0;
        self.configs = self.restore_table_configs(&session.table_states)?;
        self.selected_table = session
            .selected_table_name
            .as_deref()
            .and_then(|table_name| {
                self.tables
                    .iter()
                    .position(|table| table.name == table_name)
            })
            .unwrap_or(0);
        self.selected_row = session.selected_row;
        self.row_offset = session.row_offset;
        self.schema_offset = session.schema_offset;
        self.refresh_preview()?;

        if let (Some(table_name), Some(rowid)) = (
            self.selected_table_name().map(str::to_owned),
            session.selected_row_rowid,
        ) && let Some(row_offset) = self.db_ref()?.locate_row_offset(
            &table_name,
            rowid,
            &self.current_sort_clauses(),
            &self.current_filter_clauses(),
        )? {
            self.selected_row = row_offset;
        }

        let queried_offset = self.row_offset;
        self.clamp_row_viewport();
        if self.row_offset != queried_offset {
            self.refresh_preview()?;
        }
        self.clamp_schema_offset();
        self.ensure_sql_viewport();
        Ok(())
    }

    pub(in crate::app) fn persist_session_state(&self) -> Result<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        let Some(db) = &self.db else {
            return Ok(());
        };

        let selected_row_rowid = if let Some(table_name) = self.selected_table_name() {
            db.row_record_at_offset(
                table_name,
                &self.current_sort_clauses(),
                &self.current_filter_clauses(),
                self.selected_row,
            )?
            .and_then(|record| record.rowid)
        } else {
            None
        };

        let mut table_states = Vec::new();
        for (table_name, config) in &self.configs {
            let columns = match db.column_info(table_name) {
                Ok(columns) => columns,
                Err(_) => continue,
            };
            let hidden_columns = columns
                .iter()
                .zip(
                    config
                        .visible_columns
                        .iter()
                        .copied()
                        .chain(std::iter::repeat(true)),
                )
                .filter(|(_, visible)| !visible)
                .map(|(column, _)| column.name.clone())
                .collect::<Vec<_>>();
            let sort_rules = config
                .sort_clauses
                .iter()
                .filter_map(|rule| {
                    columns.get(rule.column_index).map(|column| StoredSortRule {
                        column_name: column.name.clone(),
                        descending: rule.descending,
                    })
                })
                .collect::<Vec<_>>();
            let filter_rules = config
                .filter_rules
                .iter()
                .filter_map(|rule| {
                    columns
                        .get(rule.column_index)
                        .map(|column| StoredFilterRule {
                            column_name: column.name.clone(),
                            mode: rule.mode,
                            value: rule.value.clone(),
                        })
                })
                .collect::<Vec<_>>();

            if hidden_columns.is_empty() && sort_rules.is_empty() && filter_rules.is_empty() {
                continue;
            }

            table_states.push(StoredTableState {
                table_name: table_name.clone(),
                hidden_columns,
                sort_rules,
                filter_rules,
            });
        }

        AppStorage::save_session(
            path,
            &StoredSession {
                mode: self.mode,
                focus: self.focus,
                content_view: self.content_view,
                selected_table_name: self.selected_table_name().map(str::to_owned),
                selected_row: self.selected_row,
                selected_row_rowid,
                row_offset: self.row_offset,
                schema_offset: self.schema_offset,
                sql_query: self.sql.query.clone(),
                sql_cursor: self.sql.cursor.min(self.sql.query.len()),
                sql_focus: self.sql.focus,
                sql_history: self.sql.history.clone(),
                table_states,
            },
        )
    }

    fn restore_table_configs(
        &self,
        stored_tables: &[StoredTableState],
    ) -> Result<HashMap<String, TableConfig>> {
        let mut configs = HashMap::new();
        let Some(db) = &self.db else {
            return Ok(configs);
        };

        for table_state in stored_tables {
            if !self
                .tables
                .iter()
                .any(|table| table.name == table_state.table_name)
            {
                continue;
            }

            let columns = db.column_info(&table_state.table_name)?;
            let visible_columns = columns
                .iter()
                .map(|column| {
                    !table_state
                        .hidden_columns
                        .iter()
                        .any(|hidden| hidden == &column.name)
                })
                .collect::<Vec<_>>();
            let sort_clauses = table_state
                .sort_rules
                .iter()
                .filter_map(|rule| {
                    columns
                        .iter()
                        .position(|column| column.name == rule.column_name)
                        .map(|column_index| SortRule {
                            column_index,
                            descending: rule.descending,
                        })
                })
                .collect();
            let filter_rules = table_state
                .filter_rules
                .iter()
                .filter_map(|rule| {
                    columns
                        .iter()
                        .position(|column| column.name == rule.column_name)
                        .map(|column_index| FilterRule {
                            column_index,
                            mode: rule.mode,
                            value: rule.value.clone(),
                        })
                })
                .collect();

            configs.insert(
                table_state.table_name.clone(),
                TableConfig {
                    visible_columns,
                    sort_clauses,
                    filter_rules,
                },
            );
        }

        Ok(configs)
    }
}
