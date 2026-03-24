use crate::db::{FilterClause, FilterMode, SortClause};

use super::App;

#[derive(Clone, Debug, Default)]
pub struct TableConfig {
    pub visible_columns: Vec<bool>,
    pub sort_clauses: Vec<SortRule>,
    pub filter_rules: Vec<FilterRule>,
}

#[derive(Clone, Debug)]
pub struct SortRule {
    pub column_index: usize,
    pub descending: bool,
}

#[derive(Clone, Debug)]
pub struct FilterRule {
    pub column_index: usize,
    pub mode: FilterMode,
    pub value: String,
}

impl App {
    pub(super) fn current_config(&self) -> Option<&TableConfig> {
        self.selected_table_name()
            .and_then(|name| self.configs.get(name))
    }

    pub(super) fn current_config_mut(&mut self) -> Option<&mut TableConfig> {
        let table_name = self.selected_table_name()?.to_string();
        self.configs.get_mut(&table_name)
    }

    pub(super) fn ensure_table_config(&mut self) {
        let Some(table_name) = self.selected_table_name().map(str::to_owned) else {
            return;
        };
        let Some(details) = &self.details else {
            return;
        };

        let entry = self.configs.entry(table_name).or_default();
        if entry.visible_columns.len() != details.columns.len() {
            entry.visible_columns = vec![true; details.columns.len()];
            entry
                .sort_clauses
                .retain(|rule| rule.column_index < details.columns.len());
            entry
                .filter_rules
                .retain(|rule| rule.column_index < details.columns.len());
        }
    }

    pub(super) fn visible_column_flags(&self) -> Vec<bool> {
        let Some(details) = &self.details else {
            return vec![];
        };
        self.current_config()
            .map(|config| config.visible_columns.clone())
            .unwrap_or_else(|| vec![true; details.columns.len()])
    }

    pub(super) fn visible_column_names(&self) -> Vec<String> {
        let Some(details) = &self.details else {
            return vec![];
        };

        details
            .columns
            .iter()
            .zip(self.visible_column_flags())
            .filter_map(|(column, visible)| visible.then(|| column.name.clone()))
            .collect()
    }

    pub(super) fn current_sort_rules(&self) -> Vec<SortRule> {
        self.current_config()
            .map(|config| config.sort_clauses.clone())
            .unwrap_or_default()
    }

    pub(super) fn current_filter_rules(&self) -> Vec<FilterRule> {
        self.current_config()
            .map(|config| config.filter_rules.clone())
            .unwrap_or_default()
    }

    pub(super) fn current_sort_clauses(&self) -> Vec<SortClause> {
        let Some(details) = &self.details else {
            return vec![];
        };

        self.current_sort_rules()
            .into_iter()
            .filter_map(|rule| {
                details
                    .columns
                    .get(rule.column_index)
                    .map(|column| SortClause {
                        column_name: column.name.clone(),
                        descending: rule.descending,
                    })
            })
            .collect()
    }

    pub(super) fn current_filter_clauses(&self) -> Vec<FilterClause> {
        let Some(details) = &self.details else {
            return vec![];
        };

        self.current_filter_rules()
            .into_iter()
            .filter_map(|rule| {
                details
                    .columns
                    .get(rule.column_index)
                    .map(|column| FilterClause {
                        column_name: column.name.clone(),
                        mode: rule.mode,
                        value: rule.value.clone(),
                    })
            })
            .collect()
    }

    pub fn hidden_column_count(&self) -> usize {
        self.current_config()
            .map(|config| {
                config
                    .visible_columns
                    .iter()
                    .filter(|visible| !**visible)
                    .count()
            })
            .unwrap_or(0)
    }

    pub(super) fn sort_summary(&self) -> String {
        let Some(details) = &self.details else {
            return String::new();
        };

        let rules = self.current_sort_rules();
        if rules.is_empty() {
            return String::new();
        }

        let parts: Vec<String> = rules
            .iter()
            .take(2)
            .filter_map(|rule| {
                details.columns.get(rule.column_index).map(|column| {
                    let direction = if rule.descending { "desc" } else { "asc" };
                    format!("{} {direction}", column.name)
                })
            })
            .collect();

        if rules.len() > 2 {
            format!("sort: {} +{}", parts.join(", "), rules.len() - 2)
        } else {
            format!("sort: {}", parts.join(", "))
        }
    }

    pub(super) fn filter_summary(&self) -> String {
        let Some(details) = &self.details else {
            return String::new();
        };

        let rules = self.current_filter_rules();
        if rules.is_empty() {
            return String::new();
        }

        let parts: Vec<String> = rules
            .iter()
            .take(2)
            .filter_map(|rule| {
                details.columns.get(rule.column_index).map(|column| {
                    let operator = super::state::filter_mode_label(rule.mode);
                    if matches!(rule.mode, FilterMode::IsTrue | FilterMode::IsFalse) {
                        format!("{} {operator}", column.name)
                    } else {
                        format!(
                            "{}{operator}{}",
                            column.name,
                            truncate_filter_value(&rule.value)
                        )
                    }
                })
            })
            .collect();

        if rules.len() > 2 {
            format!("filter: {} +{}", parts.join(", "), rules.len() - 2)
        } else {
            format!("filter: {}", parts.join(", "))
        }
    }
}

fn truncate_filter_value(value: &str) -> String {
    const MAX: usize = 12;
    let mut chars = value.chars();
    let truncated: String = chars.by_ref().take(MAX).collect();
    if chars.next().is_some() {
        format!("{truncated}…")
    } else {
        truncated
    }
}
