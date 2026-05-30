use sqlformat::{FormatOptions, QueryParams};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::db::FilterMode;

use super::{App, AppMode, FilterPane, ModalPane, SearchScope, home::recent_path_label};

pub struct HelpEntry {
    pub key: String,
    pub description: String,
}

const HOME_LOGO: &str = concat!(
    " ▄▄▄▄▄▄▄   ▄▄▄▄▄   ▄▄▄  ▄▄▄ ▄▄▄▄▄ ▄▄▄▄▄▄\n",
    "█████▀▀▀ ▄███████▄ ███  ███  ███  ███▀▀██▄\n",
    " ▀████▄  ███   ███ ███  ███  ███  ███  ███\n",
    "   ▀████ ███▄█▄███ ███▄▄███  ███  ███  ███\n",
    "███████▀  ▀█████▀  ▀██████▀ ▄███▄ ██████▀\n",
    "               ▀▀"
);

impl App {
    pub fn schema_lines(&self) -> Vec<String> {
        let key = self.schema_cache_key();
        if let Some((cached_key, cached_lines)) = self.schema_lines_cache.borrow().as_ref() {
            if *cached_key == key {
                return cached_lines.clone();
            }
        }

        let lines = self.build_schema_lines();
        *self.schema_lines_cache.borrow_mut() = Some((key, lines.clone()));
        lines
    }

    fn build_schema_lines(&self) -> Vec<String> {
        if self.is_home() {
            return self.home_screen_lines();
        }

        let Some(details) = &self.details else {
            return vec!["No schema details available".to_string()];
        };
        let table_label = self
            .selected_table_label()
            .unwrap_or_else(|| "-".to_string());

        let mut lines = vec![
            format!("Table: {table_label}"),
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
        if self.show_help {
            return "Esc or ? close help".to_string();
        }

        if self.settings_open() {
            return self.settings_footer_hint();
        }

        let compact = self.compact_footer_hint();
        if self.help_available() {
            if compact.is_empty() {
                "? help".to_string()
            } else {
                format!("{compact}  ? help")
            }
        } else {
            compact
        }
    }

    pub fn help_available(&self) -> bool {
        !self.settings_open()
            && !(self.detail_is_editing()
                || self.mode == AppMode::Sql && self.sql_focus() == super::SqlPane::Editor)
            && self.filter_modal_pane() != Some(FilterPane::Draft)
    }

    pub fn overlay_modal_open(&self) -> bool {
        self.detail.is_some() || self.filter_modal.is_some() || self.modal.is_some()
    }

    pub fn help_title(&self) -> &'static str {
        if self.is_home() {
            "Home Controls"
        } else if self.mode == AppMode::Sql {
            "SQL Controls"
        } else if self.detail.is_some() {
            "Row Detail Controls"
        } else if self.filter_modal.is_some() {
            "Filter Controls"
        } else if self.modal.is_some() {
            "View Controls"
        } else if self.search.is_some() {
            "Search Controls"
        } else {
            "Browse Controls"
        }
    }

    pub fn help_entries(&self) -> Vec<HelpEntry> {
        if self.is_home() {
            return self.home_help_entries();
        }

        if self.mode == AppMode::Sql {
            return vec![
                entry("1 / 2", "Browse / SQL mode"),
                entry(",", "Settings"),
                entry("Tab", "Cycle panes"),
                entry("F5", "Run query"),
                entry("Enter", "New line / apply completion"),
                entry("↑/↓", "Scroll pane"),
                entry("PgUp/PgDn", "Scroll results"),
                entry("c", "Clear history or results"),
                entry("q", "Quit"),
            ];
        }

        if self.detail.is_some() {
            return self.detail_help_entries();
        }

        if self.filter_modal.is_some() {
            return vec![
                entry("Esc / q", "Close"),
                entry("←/→", "Switch pane"),
                entry("↑/↓", "Move selection"),
                entry("Type", "Edit filter value"),
                entry("Enter", "Apply filter"),
                entry("Del", "Remove filter"),
                entry("Space", "Toggle / cycle mode"),
            ];
        }

        if self.modal.is_some() {
            return vec![
                entry("Esc / q", "Close"),
                entry("←/→", "Switch pane"),
                entry("Space", "Toggle column visibility"),
                entry("Enter", "Add / update sort"),
                entry("Del", "Remove sort"),
                entry("c", "Clear sorts"),
                entry("M", "Open filters"),
            ];
        }

        if let Some(search) = &self.search {
            return self.search_help_entries(search);
        }

        self.browse_help_entries()
    }

    fn compact_footer_hint(&self) -> String {
        if self.is_home() {
            if self.pending_recent_removal.is_some() {
                return "↑↓  Enter  Del confirm".to_string();
            }
            return "↑↓  Enter  , settings".to_string();
        }

        if self.mode == AppMode::Sql {
            return "Tab  F5 run".to_string();
        }

        if self.detail.is_some() {
            if self.detail_is_editing() {
                return "Esc done  Enter newline".to_string();
            }
            if self.detail_has_changes() {
                return "e edit  ↑↓ field  ←→ panes".to_string();
            }
            if self.detail.as_ref().is_some_and(|d| d.is_new_row) {
                return "e edit  s insert".to_string();
            }
            if self.detail_is_row_writable() && self.can_delete_detail_row() {
                return "e edit  d delete  g link".to_string();
            }
            if self.detail_is_row_writable() {
                return "e edit  g link".to_string();
            }
            return "Read-only  g link".to_string();
        }

        if self.filter_modal.is_some() {
            return "Esc close  Enter apply".to_string();
        }

        if self.modal.is_some() {
            return "Esc close  Enter sort".to_string();
        }

        if let Some(search) = &self.search {
            if search.loading {
                let scope = match search.scope {
                    SearchScope::CurrentTable => "table",
                    SearchScope::AllTables => "all",
                };
                return format!("Searching {scope}…");
            }
            return "Type  ↑↓ select  Enter jump".to_string();
        }

        "Tab  ↑↓ move  Enter  , settings".to_string()
    }

    fn home_help_entries(&self) -> Vec<HelpEntry> {
        let delete_label = if self.pending_recent_removal.is_some() {
            "Confirm remove from recents"
        } else {
            "Remove from recents"
        };

        vec![
            entry("↑/↓", "Move selection"),
            entry("Enter", "Open database"),
            entry(",", "Settings"),
            entry("Del", delete_label),
            entry("r", "Reload recents"),
            entry("q", "Quit"),
        ]
    }

    fn browse_help_entries(&self) -> Vec<HelpEntry> {
        let mut entries = vec![
            entry("1 / 2", "Browse / SQL mode"),
            entry(",", "Settings"),
            entry("Tab / ←/→", "Switch panes"),
            entry("↑/↓", "Move selection"),
            entry("Enter", "Open row details"),
            entry("f", "Search current table"),
            entry("F", "Search all tables"),
            entry("v", "Toggle rows / schema"),
            entry("m", "Sort & columns"),
            entry("M", "Filters"),
            entry("r", "Reload"),
            entry("q", "Quit"),
        ];

        if self.can_add_new_row() {
            entries.insert(5, entry("a", "New row"));
        }
        if self.can_delete_row() {
            let idx = if self.can_add_new_row() { 6 } else { 5 };
            entries.insert(idx, entry("d", "Delete row"));
        }

        entries
    }

    fn detail_help_entries(&self) -> Vec<HelpEntry> {
        if self.detail_is_editing() {
            return vec![
                entry("Esc", "Stop editing"),
                entry("Type", "Edit value"),
                entry("Enter", "New line"),
                entry("Backspace", "Delete character"),
            ];
        }

        let mut entries = vec![
            entry("Esc / q", "Close"),
            entry("←/→", "Switch pane"),
            entry("↑/↓", "Move field"),
            entry("Wheel / ↑/↓", "Scroll value pane"),
            entry("e", "Edit field"),
            entry("g", "Follow foreign key"),
        ];

        if self.detail_has_changes() {
            let save = if self.detail.as_ref().is_some_and(|d| d.is_new_row) {
                "Insert row"
            } else {
                "Save row"
            };
            entries.push(entry("s", save));
            entries.push(entry("c", "Discard changes"));
        } else if self.detail.as_ref().is_some_and(|d| d.is_new_row) {
            entries.push(entry("s", "Insert row"));
        }

        if self.can_delete_detail_row()
            && !self.detail.as_ref().is_some_and(|d| d.is_new_row)
            && !self.detail_has_changes()
        {
            entries.push(entry("d", "Delete row"));
        }

        entries
    }

    fn search_help_entries(&self, search: &super::SearchState) -> Vec<HelpEntry> {
        let scope = match search.scope {
            SearchScope::CurrentTable => "current table",
            SearchScope::AllTables => "all tables",
        };

        if search.loading {
            return vec![entry("…", format!("Searching {scope}"))];
        }

        let mut entries = vec![
            entry("Esc", "Close search"),
            entry("↑/↓", "Select result"),
            entry("Enter", "Jump to result"),
            entry("Backspace", "Delete character"),
        ];

        match search.scope {
            SearchScope::CurrentTable if self.current_table_search_is_live() => {
                entries.insert(1, entry("Type", "Filter results"));
            }
            SearchScope::CurrentTable if search.submitted => {
                entries.insert(1, entry("Type", "Edit query, Enter to rerun"));
            }
            SearchScope::CurrentTable => {
                entries.insert(1, entry("Type", "Enter query to run"));
            }
            SearchScope::AllTables => {
                entries.insert(1, entry("Type", "Enter query to run"));
                entries.insert(3, entry("←/→", "Scroll results"));
            }
        }

        entries
    }

    pub fn content_title(&self) -> String {
        if self.is_home() {
            return "Home".to_string();
        }

        let table = self
            .selected_table_label()
            .unwrap_or_else(|| "Rows".to_string());
        let hidden = self.hidden_column_count();
        let filters = self.filter_summary();
        let sort = self.sort_summary();

        let mut parts = vec![table];
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
                let dirty = if field.is_dirty() { "* " } else { "  " };
                let suffix = if field.is_blob {
                    " [blob]"
                } else if field.foreign_target.is_some() {
                    " ->"
                } else {
                    ""
                };
                format!("{dirty}{}{}", field.column_name, suffix)
            })
            .collect()
    }

    pub fn home_status_line(&self) -> Option<String> {
        if !self.is_home() {
            return None;
        }

        if let Some(status) = &self.status_message {
            Some(status.clone())
        } else {
            self.selected_recent_item().map(|item| {
                if item.available {
                    format!("Selected: {}", item.path.display())
                } else {
                    format!("Missing: {}", item.path.display())
                }
            })
        }
    }

    pub fn home_recent_lines(&self) -> Vec<String> {
        if self.recent_items.is_empty() {
            vec!["No recent files".to_string()]
        } else {
            self.recent_items
                .iter()
                .map(|item| {
                    let mut label = recent_path_label(&item.path);
                    if item.available {
                        label
                    } else {
                        label.push_str(" [missing]");
                        label
                    }
                })
                .collect()
        }
    }

    pub fn home_logo_lines(&self) -> Vec<String> {
        HOME_LOGO.lines().map(str::to_string).collect()
    }

    fn home_screen_lines(&self) -> Vec<String> {
        self.home_logo_lines()
    }

    pub fn format_cell_preview(&self, value: &str) -> String {
        truncate_cell_preview(value, self.app_settings.cell_preview_max_chars)
    }

    pub(crate) fn schema_cache_key(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.app_settings.color_scheme.hash(&mut hasher);
        self.is_home().hash(&mut hasher);
        self.selected_table.hash(&mut hasher);
        self.tables.len().hash(&mut hasher);
        self.selected_table_label().hash(&mut hasher);
        self.details.as_ref().map(|details| {
            details.total_rows.hash(&mut hasher);
            details.columns.len().hash(&mut hasher);
            for column in &details.columns {
                column.name.hash(&mut hasher);
                column.data_type.hash(&mut hasher);
                column.not_null.hash(&mut hasher);
                column.is_primary_key.hash(&mut hasher);
                column.default_value.hash(&mut hasher);
            }
            details.create_sql.hash(&mut hasher);
        });
        hasher.finish()
    }
}

fn empty_as_unknown(value: &str) -> &str {
    if value.is_empty() { "UNKNOWN" } else { value }
}

fn entry(key: impl Into<String>, description: impl Into<String>) -> HelpEntry {
    HelpEntry {
        key: key.into(),
        description: description.into(),
    }
}

pub(crate) fn truncate_cell_preview(value: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return value.to_string();
    }

    let char_count = value.chars().count();
    if char_count <= max_chars {
        return value.to_string();
    }

    let truncated: String = value.chars().take(max_chars).collect();
    format!("{truncated}…")
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

#[cfg(test)]
mod tests;
