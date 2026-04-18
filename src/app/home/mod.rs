mod path;
mod storage;

pub use storage::{RecentItem, RecentStore};

pub(super) use path::{normalize_database_path, recent_path_is_available};
#[cfg(test)]
pub(super) use path::{path_to_sqlite_uri_path, recent_paths_match};
pub(super) use storage::{
    AppStorage, StoredFilterRule, StoredSession, StoredSortRule, StoredTableState,
};
#[cfg(all(test, unix))]
pub(super) use storage::{path_from_storage_bytes, path_to_storage_bytes};

#[cfg(test)]
mod tests;
