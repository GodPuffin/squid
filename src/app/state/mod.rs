use std::{collections::HashMap, sync::Arc};

use crate::db::{FilterMode, SearchHit};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AppMode {
    Home,
    Browse,
    Sql,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SearchScope {
    CurrentTable,
    AllTables,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SqlPane {
    Editor,
    History,
    Results,
}

#[derive(Clone, Debug)]
pub struct SqlHistoryEntry {
    pub query: String,
    pub summary: String,
}

#[derive(Clone, Debug)]
pub struct SqlCompletionItem {
    pub label: String,
    pub insert_text: String,
}

#[derive(Clone, Debug)]
pub struct SqlCompletionState {
    pub prefix_start: usize,
    pub items: Vec<SqlCompletionItem>,
    pub selected: usize,
}

#[derive(Clone, Debug)]
pub enum SqlResultState {
    Empty,
    Rows {
        columns: Vec<String>,
        rows: Vec<Vec<String>>,
    },
    Message {
        text: String,
        is_error: bool,
    },
}

#[derive(Clone, Debug)]
pub struct SqlState {
    pub query: String,
    pub cursor: usize,
    pub editor_scroll: usize,
    pub editor_col_offset: usize,
    pub editor_height: usize,
    pub editor_width: usize,
    pub focus: SqlPane,
    pub history: Vec<SqlHistoryEntry>,
    pub history_offset: usize,
    pub history_height: usize,
    pub selected_history: usize,
    pub result: SqlResultState,
    pub result_scroll: usize,
    pub result_height: usize,
    pub completion: Option<SqlCompletionState>,
    pub status: String,
    pub column_cache: HashMap<String, Arc<[String]>>,
    pub completion_cache_query: String,
    pub completion_candidates_cache: HashMap<String, Vec<SqlCompletionItem>>,
}

#[derive(Clone, Debug)]
pub struct SearchState {
    pub scope: SearchScope,
    pub query: String,
    pub results: Vec<SearchHit>,
    pub selected_result: usize,
    pub result_offset: usize,
    pub result_limit: usize,
    pub submitted: bool,
    pub loading: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModalPane {
    Columns,
    SortColumns,
    SortActive,
}

#[derive(Clone, Debug)]
pub struct ModalState {
    pub pane: ModalPane,
    pub column_index: usize,
    pub sort_column_index: usize,
    pub sort_active_index: usize,
    pub pending_desc: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FilterPane {
    Columns,
    Modes,
    Draft,
    Active,
}

#[derive(Clone, Debug)]
pub struct FilterModalState {
    pub pane: FilterPane,
    pub column_index: usize,
    pub mode_index: usize,
    pub active_index: usize,
    pub input: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DetailPane {
    Fields,
    Value,
}

#[derive(Clone, Debug)]
pub struct DetailMessage {
    pub text: String,
    pub is_error: bool,
}

#[derive(Clone, Debug)]
pub struct DetailForeignTarget {
    pub table_name: String,
    pub column_name: String,
    pub value: String,
}

#[derive(Clone, Debug)]
pub struct DetailField {
    pub column_name: String,
    pub data_type: String,
    pub not_null: bool,
    pub original_value: String,
    pub draft_value: String,
    pub foreign_target: Option<DetailForeignTarget>,
    pub is_blob: bool,
}

impl DetailField {
    pub fn is_dirty(&self) -> bool {
        self.original_value != self.draft_value
    }
}

#[derive(Clone, Debug)]
pub struct DetailState {
    pub rowid: Option<i64>,
    pub row_label: String,
    pub pane: DetailPane,
    pub selected_field: usize,
    pub value_scroll: usize,
    pub value_view_width: usize,
    pub value_view_height: usize,
    pub is_editing: bool,
    pub message: Option<DetailMessage>,
    pub fields: Vec<DetailField>,
}

pub fn filter_mode_label(mode: FilterMode) -> &'static str {
    match mode {
        FilterMode::Contains => "~",
        FilterMode::Equals => "=",
        FilterMode::StartsWith => "^",
        FilterMode::GreaterThan => ">",
        FilterMode::LessThan => "<",
        FilterMode::IsTrue => "is true",
        FilterMode::IsFalse => "is false",
    }
}
