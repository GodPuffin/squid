mod core;
pub(crate) mod detail;
mod filter;
mod home;
mod modal;
mod navigation;
mod presenter;
mod search;
mod sql;
mod state;
mod table_config;

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::db::{Database, DeferredSearchWork, RowPreview, TableDetails, TableSummary};
pub use home::{RecentItem, RecentStore};

pub use state::{
    AppMode, DetailField, DetailForeignTarget, DetailMessage, DetailPane, DetailState,
    FilterModalState, FilterPane, ModalPane, ModalState, SearchScope, SearchState,
    SqlCompletionItem, SqlCompletionState, SqlHistoryEntry, SqlPane, SqlResultState, SqlState,
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
    SwitchToBrowse,
    SwitchToSql,
    ToggleFocus,
    ReverseFocus,
    ToggleView,
    MoveUp,
    MoveDown,
    MoveLeft,
    MoveRight,
    MoveHome,
    MoveEnd,
    PageUp,
    PageDown,
    OpenConfig,
    CloseModal,
    ToggleItem,
    Confirm,
    FollowLink,
    EditDetail,
    SaveDetail,
    DiscardDetail,
    Delete,
    Clear,
    Reload,
    OpenSearchCurrent,
    OpenSearchAll,
    OpenFilters,
    ExecuteSql,
    NewLine,
    InputChar(char),
    Backspace,
}

pub struct App {
    pub mode: AppMode,
    path: Option<PathBuf>,
    pub db: Option<Database>,
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
    pub(crate) pending_search: Option<DeferredSearchWork>,
    pub search_results_view_width: usize,
    pub recent_items: Vec<RecentItem>,
    pub selected_recent: usize,
    pub status_message: Option<String>,
    pub sql: SqlState,
    configs: HashMap<String, TableConfig>,
    schema_lines_cache: RefCell<Option<(u64, Vec<String>)>>,
}

impl Drop for App {
    fn drop(&mut self) {
        let _ = self.persist_session_state();
    }
}
