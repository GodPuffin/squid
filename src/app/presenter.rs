use sqlformat::{FormatOptions, QueryParams};

use crate::db::FilterMode;

use super::{App, FilterPane, ModalPane, SearchScope};

impl App {
    pub fn schema_lines(&self) -> Vec<String> {
        let Some(details) = &self.details else {
            return vec!["No schema details available".to_string()];
        };

        let mut lines = vec![
            format!("Table: {}", self.selected_table_name().unwrap_or("-")),
            format!("Rows: {}", details.total_rows),
            String::new(),
            format!("Columns ({})", details.columns.len()),
        ];

        lines.extend(details.columns.iter().map(|column| {
            let nullable = if column.not_null { "NOT NULL" } else { "NULL" };
            let primary_key = if column.is_primary_key { " PK" } else { "" };
            let default = column.default_value.as_deref().unwrap_or("-");
            format!(
                "{} | {} | {}{} | default {}",
                column.name,
                empty_as_unknown(&column.data_type),
                nullable,
                primary_key,
                default
            )
        }));

        if let Some(sql) = &details.create_sql {
            lines.push(String::new());
            lines.push("Create SQL".to_string());
            lines.extend(format_create_sql(sql));
        }

        lines
    }

    pub fn footer_hint(&self) -> String {
        if self.detail.is_some() {
            "Esc/q close  Up/Down field  Left/Right pane  Wheel or Up/Down in value pane scroll  g follow foreign key".to_string()
        } else if self.filter_modal.is_some() {
            "Esc close  q close outside value input  Left/Right switch pane  Up/Down move  Type value  Enter apply  Delete remove  Space cycle operator".to_string()
        } else if self.modal.is_some() {
            "Esc/q close  Left/Right switch pane  Space toggle  Enter add/update sort  Delete remove sort  c clear sorts  M filters".to_string()
        } else if let Some(search) = &self.search {
            let scope = match search.scope {
                SearchScope::CurrentTable => "current table",
                SearchScope::AllTables => "all tables",
            };
            match search.scope {
                SearchScope::CurrentTable => format!(
                    "Search {scope}  Type to filter  Up/Down select  Enter jump  Esc close  Backspace delete"
                ),
                SearchScope::AllTables => format!(
                    "Search {scope}  Type query then Enter to run  Up/Down select  Enter jump  Esc close"
                ),
            }
        } else {
            "Left/Right or Tab pane  Up/Down move  Enter row details  f search table  F search all  v rows/schema  m sort  M filters  r reload  q quit".to_string()
        }
    }

    pub fn content_title(&self) -> String {
        let table = self.selected_table_name().unwrap_or("Rows");
        let hidden = self.hidden_column_count();
        let filters = self.filter_summary();
        let sort = self.sort_summary();

        let mut parts = vec![table.to_string()];
        if hidden > 0 {
            parts.push(format!("+{hidden} hidden"));
        }
        if !filters.is_empty() {
            parts.push(filters);
        }
        if !sort.is_empty() {
            parts.push(sort);
        }
        parts.join("  ")
    }

    pub fn modal_pane(&self) -> Option<ModalPane> {
        self.modal.as_ref().map(|modal| modal.pane)
    }

    pub fn modal_column_lines(&self) -> Vec<String> {
        let Some(details) = &self.details else {
            return vec![];
        };

        details
            .columns
            .iter()
            .zip(self.visible_column_flags())
            .map(|(column, is_visible)| {
                let marker = if is_visible { "[x]" } else { "[ ]" };
                format!("{marker} {}", column.name)
            })
            .collect()
    }

    pub fn modal_sort_column_lines(&self) -> Vec<String> {
        let Some(details) = &self.details else {
            return vec![];
        };
        let Some(modal) = &self.modal else {
            return vec![];
        };

        details
            .columns
            .iter()
            .enumerate()
            .map(|(idx, column)| {
                let direction = if idx == modal.sort_column_index {
                    if modal.pending_desc { "DESC" } else { "ASC" }
                } else {
                    ""
                };
                if direction.is_empty() {
                    column.name.clone()
                } else {
                    format!("{} ({direction})", column.name)
                }
            })
            .collect()
    }

    pub fn modal_sort_active_lines(&self) -> Vec<String> {
        let Some(details) = &self.details else {
            return vec![];
        };
        let rules = self.current_sort_rules();
        if rules.is_empty() {
            return vec!["No active sort".to_string()];
        }

        rules
            .iter()
            .enumerate()
            .map(|(idx, rule)| {
                let name = details
                    .columns
                    .get(rule.column_index)
                    .map(|column| column.name.as_str())
                    .unwrap_or("?");
                let direction = if rule.descending { "DESC" } else { "ASC" };
                format!("{}. {} {direction}", idx + 1, name)
            })
            .collect()
    }

    pub fn modal_selected_indices(&self) -> (Option<usize>, Option<usize>, Option<usize>) {
        let Some(modal) = &self.modal else {
            return (None, None, None);
        };

        let active_len = self.current_sort_rules().len();
        let active_index = if active_len == 0 {
            None
        } else {
            Some(modal.sort_active_index.min(active_len.saturating_sub(1)))
        };

        (
            self.details.as_ref().map(|_| modal.column_index),
            self.details.as_ref().map(|_| modal.sort_column_index),
            active_index,
        )
    }

    pub fn search_selected_index_in_view(&self) -> Option<usize> {
        let Some(search) = &self.search else {
            return None;
        };
        search
            .selected_result
            .checked_sub(search.result_offset)
            .filter(|index| *index < search.results.len())
    }

    pub fn search_headers(&self) -> Vec<String> {
        self.visible_column_names()
    }

    pub fn modal_filter_column_name(&self) -> String {
        let Some(details) = &self.details else {
            return "-".to_string();
        };
        let Some(modal) = &self.filter_modal else {
            return "-".to_string();
        };

        details
            .columns
            .get(modal.column_index)
            .map(|column| column.name.clone())
            .unwrap_or_else(|| "-".to_string())
    }

    pub fn modal_filter_mode(&self) -> FilterMode {
        self.active_filter_mode().unwrap_or(FilterMode::Contains)
    }

    pub fn modal_filter_input(&self) -> &str {
        self.filter_modal
            .as_ref()
            .map(|modal| modal.input.as_str())
            .unwrap_or("")
    }

    pub fn modal_filter_active_lines(&self) -> Vec<String> {
        let Some(details) = &self.details else {
            return vec![];
        };
        let rules = self.current_filter_rules();
        if rules.is_empty() {
            return vec!["No active filters".to_string()];
        }

        rules
            .iter()
            .map(|rule| {
                let name = details
                    .columns
                    .get(rule.column_index)
                    .map(|column| column.name.as_str())
                    .unwrap_or("?");
                format!(
                    "{name} {} {}",
                    super::state::filter_mode_label(rule.mode),
                    rule.value
                )
            })
            .collect()
    }

    pub fn filter_modal_pane(&self) -> Option<FilterPane> {
        self.filter_modal.as_ref().map(|modal| modal.pane)
    }

    pub fn detail_field_lines(&self) -> Vec<String> {
        let Some(detail) = &self.detail else {
            return vec![];
        };

        detail
            .fields
            .iter()
            .map(|field| {
                if field.foreign_target.is_some() {
                    format!("{}  ->", field.column_name)
                } else {
                    field.column_name.clone()
                }
            })
            .collect()
    }
}

fn empty_as_unknown(value: &str) -> &str {
    if value.is_empty() { "UNKNOWN" } else { value }
}

fn format_create_sql(sql: &str) -> Vec<String> {
    let formatted = sqlformat::format(sql, &QueryParams::None, &FormatOptions::default());
    let lines: Vec<String> = formatted.lines().map(str::to_string).collect();

    if lines.is_empty() {
        vec![sql.to_string()]
    } else {
        lines
    }
}
