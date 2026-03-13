use crate::db::{FilterMode, SearchHit};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SearchScope {
    CurrentTable,
    AllTables,
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
pub struct DetailForeignTarget {
    pub table_name: String,
    pub column_name: String,
    pub value: String,
}

#[derive(Clone, Debug)]
pub struct DetailField {
    pub column_name: String,
    pub value: String,
    pub foreign_target: Option<DetailForeignTarget>,
}

#[derive(Clone, Debug)]
pub struct DetailState {
    pub row_label: String,
    pub pane: DetailPane,
    pub selected_field: usize,
    pub value_scroll: usize,
    pub value_view_width: usize,
    pub value_view_height: usize,
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
