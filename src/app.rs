mod core;
mod detail;
mod filter;
mod modal;
mod navigation;
mod presenter;
mod search;
mod state;
mod table_config;

use std::collections::HashMap;
use std::path::PathBuf;

use crate::db::{Database, RowPreview, TableDetails, TableSummary};

pub use state::{
    DetailField, DetailForeignTarget, DetailPane, DetailState, FilterModalState, FilterPane,
    ModalPane, ModalState, SearchScope, SearchState,
};
pub use table_config::{FilterRule, SortRule, TableConfig};

const DEFAULT_ROW_LIMIT: usize = 25;
const DEFAULT_SCHEMA_PAGE_LINES: usize = 20;
const DEFAULT_DETAIL_VALUE_WIDTH: usize = 40;
const DEFAULT_DETAIL_VALUE_HEIGHT: usize = 10;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PaneFocus {
    Tables,
    Content,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContentView {
    Rows,
    Schema,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    None,
    Quit,
    ToggleFocus,
    ToggleView,
    MoveUp,
    MoveDown,
    MoveLeft,
    MoveRight,
    OpenConfig,
    CloseModal,
    ToggleItem,
    Confirm,
    FollowLink,
    Delete,
    Clear,
    Reload,
    OpenSearchCurrent,
    OpenSearchAll,
    OpenFilters,
    InputChar(char),
    Backspace,
}

pub struct App {
    path: PathBuf,
    pub db: Database,
    pub tables: Vec<TableSummary>,
    pub selected_table: usize,
    pub focus: PaneFocus,
    pub content_view: ContentView,
    pub row_offset: usize,
    pub row_limit: usize,
    pub selected_row: usize,
    pub schema_offset: usize,
    pub schema_page_lines: usize,
    pub preview: RowPreview,
    pub details: Option<TableDetails>,
    pub detail: Option<DetailState>,
    pub filter_modal: Option<FilterModalState>,
    pub modal: Option<ModalState>,
    pub search: Option<SearchState>,
    configs: HashMap<String, TableConfig>,
}
