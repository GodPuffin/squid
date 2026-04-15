use std::env;
use std::fs;
#[cfg(unix)]
use std::os::unix::ffi::{OsStrExt, OsStringExt};
#[cfg(windows)]
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};

use crate::db::FilterMode;

use super::{AppMode, ContentView, PaneFocus, SqlHistoryEntry, SqlPane};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecentItem {
    pub path: PathBuf,
    pub available: bool,
}

#[derive(Clone, Debug)]
pub(super) struct StoredSortRule {
    pub column_name: String,
    pub descending: bool,
}

#[derive(Clone, Debug)]
pub(super) struct StoredFilterRule {
    pub column_name: String,
    pub mode: FilterMode,
    pub value: String,
}

#[derive(Clone, Debug)]
pub(super) struct StoredTableState {
    pub table_name: String,
    pub hidden_columns: Vec<String>,
    pub sort_rules: Vec<StoredSortRule>,
    pub filter_rules: Vec<StoredFilterRule>,
}

#[derive(Clone, Debug)]
pub(super) struct StoredSession {
    pub mode: AppMode,
    pub focus: PaneFocus,
    pub content_view: ContentView,
    pub selected_table_name: Option<String>,
    pub selected_row: usize,
    pub selected_row_rowid: Option<i64>,
    pub row_offset: usize,
    pub schema_offset: usize,
    pub sql_query: String,
    pub sql_cursor: usize,
    pub sql_focus: SqlPane,
    pub sql_history: Vec<SqlHistoryEntry>,
    pub table_states: Vec<StoredTableState>,
}

pub struct RecentStore;

impl RecentStore {
    const MAX_ITEMS: usize = 10;

    pub fn load() -> Result<Vec<RecentItem>> {
        AppStorage::load_recent(Self::MAX_ITEMS)
    }

    pub fn record(path: &Path) -> Result<Vec<RecentItem>> {
        let absolute = normalize_database_path(path)?;
        AppStorage::record_recent(&absolute)?;
        Self::load()
    }

    pub fn remove(path: &Path) -> Result<Vec<RecentItem>> {
        AppStorage::remove_recent(path)?;
        Self::load()
    }
}

pub(super) struct AppStorage;

impl AppStorage {
    pub fn load_recent(limit: usize) -> Result<Vec<RecentItem>> {
        Self::load_recent_from_path(&Self::storage_path()?, limit)
    }

    fn load_recent_from_path(storage_path: &Path, limit: usize) -> Result<Vec<RecentItem>> {
        let conn = Self::open_at(storage_path)?;
        let mut stmt = conn.prepare(
            "SELECT path
             FROM recent_databases
             ORDER BY last_opened_at DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map([limit as i64], |row| row.get::<_, Vec<u8>>(0))?;

        let mut items = Vec::new();
        for row in rows {
            let path = path_from_storage_bytes(&row?)?;
            items.push(RecentItem {
                available: recent_path_is_available(&path),
                path,
            });
        }

        Ok(items)
    }

    pub fn record_recent(path: &Path) -> Result<()> {
        let normalized = normalize_database_path(path)?;
        Self::record_recent_at(&Self::storage_path()?, &normalized)
    }

    fn record_recent_at(storage_path: &Path, path: &Path) -> Result<()> {
        let normalized = normalize_database_path(path)?;
        let conn = Self::open_at(storage_path)?;
        let encoded = path_to_storage_bytes(&normalized);
        let now = unix_timestamp();
        conn.execute(
            "INSERT INTO recent_databases(path, last_opened_at)
             VALUES (?1, ?2)
             ON CONFLICT(path) DO UPDATE SET last_opened_at = excluded.last_opened_at",
            params![encoded, now],
        )?;
        conn.execute(
            "INSERT INTO app_meta(key, value)
             VALUES ('last_opened_path', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [path_to_storage_bytes(&normalized)],
        )?;
        Ok(())
    }

    pub fn remove_recent(path: &Path) -> Result<()> {
        let normalized = normalize_database_path(path)?;
        Self::remove_recent_at(&Self::storage_path()?, &normalized)
    }

    fn remove_recent_at(storage_path: &Path, path: &Path) -> Result<()> {
        let normalized = normalize_database_path(path)?;
        let conn = Self::open_at(storage_path)?;
        let encoded = path_to_storage_bytes(&normalized);
        let tx = conn.unchecked_transaction()?;
        tx.execute(
            "DELETE FROM recent_databases WHERE path = ?1",
            [encoded.clone()],
        )?;
        tx.execute("DELETE FROM sessions WHERE path = ?1", [encoded.clone()])?;
        tx.execute("DELETE FROM sql_history WHERE path = ?1", [encoded.clone()])?;
        tx.execute(
            "DELETE FROM hidden_columns WHERE path = ?1",
            [encoded.clone()],
        )?;
        tx.execute("DELETE FROM sort_rules WHERE path = ?1", [encoded.clone()])?;
        tx.execute(
            "DELETE FROM filter_rules WHERE path = ?1",
            [encoded.clone()],
        )?;

        let last_opened = tx
            .query_row(
                "SELECT value FROM app_meta WHERE key = 'last_opened_path'",
                [],
                |row| row.get::<_, Vec<u8>>(0),
            )
            .optional()?;
        if last_opened.as_deref() == Some(encoded.as_slice()) {
            tx.execute("DELETE FROM app_meta WHERE key = 'last_opened_path'", [])?;
        }

        tx.commit()?;
        Ok(())
    }

    #[cfg(test)]
    fn last_opened_path_at(storage_path: &Path) -> Result<Option<PathBuf>> {
        let conn = Self::open_at(storage_path)?;
        let bytes = conn
            .query_row(
                "SELECT value FROM app_meta WHERE key = 'last_opened_path'",
                [],
                |row| row.get::<_, Vec<u8>>(0),
            )
            .optional()?;
        bytes
            .map(|value| path_from_storage_bytes(&value))
            .transpose()
    }

    pub fn load_session(path: &Path) -> Result<Option<StoredSession>> {
        let normalized = normalize_database_path(path)?;
        Self::load_session_at(&Self::storage_path()?, &normalized)
    }

    fn load_session_at(storage_path: &Path, path: &Path) -> Result<Option<StoredSession>> {
        let normalized = normalize_database_path(path)?;
        let conn = Self::open_at(storage_path)?;
        let encoded = path_to_storage_bytes(&normalized);
        let Some(session_row) = conn
            .query_row(
                "SELECT mode, focus, content_view, selected_table_name, selected_row,
                        selected_row_rowid, row_offset, schema_offset, sql_query,
                        sql_cursor, sql_focus
                 FROM sessions
                 WHERE path = ?1",
                [encoded.clone()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, Option<i64>>(5)?,
                        row.get::<_, i64>(6)?,
                        row.get::<_, i64>(7)?,
                        row.get::<_, String>(8)?,
                        row.get::<_, i64>(9)?,
                        row.get::<_, String>(10)?,
                    ))
                },
            )
            .optional()?
        else {
            return Ok(None);
        };

        let mut history_stmt = conn.prepare(
            "SELECT query, summary
             FROM sql_history
             WHERE path = ?1
             ORDER BY position ASC",
        )?;
        let sql_history = history_stmt
            .query_map([encoded.clone()], |row| {
                Ok(SqlHistoryEntry {
                    query: row.get(0)?,
                    summary: row.get(1)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let mut hidden_stmt = conn.prepare(
            "SELECT table_name, column_name
             FROM hidden_columns
             WHERE path = ?1
             ORDER BY table_name ASC, column_name ASC",
        )?;
        let hidden_rows = hidden_stmt
            .query_map([encoded.clone()], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let mut sort_stmt = conn.prepare(
            "SELECT table_name, column_name, descending
             FROM sort_rules
             WHERE path = ?1
             ORDER BY table_name ASC, position ASC",
        )?;
        let sort_rows = sort_stmt
            .query_map([encoded.clone()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    StoredSortRule {
                        column_name: row.get(1)?,
                        descending: row.get::<_, i64>(2)? != 0,
                    },
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let mut filter_stmt = conn.prepare(
            "SELECT table_name, column_name, mode, value
             FROM filter_rules
             WHERE path = ?1
             ORDER BY table_name ASC, position ASC",
        )?;
        let filter_rows = filter_stmt
            .query_map([encoded], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let mut table_states = std::collections::BTreeMap::<String, StoredTableState>::new();
        for (table_name, column_name) in hidden_rows {
            table_states
                .entry(table_name.clone())
                .or_insert_with(|| empty_table_state(table_name))
                .hidden_columns
                .push(column_name);
        }
        for (table_name, rule) in sort_rows {
            table_states
                .entry(table_name.clone())
                .or_insert_with(|| empty_table_state(table_name))
                .sort_rules
                .push(rule);
        }
        for (table_name, column_name, mode, value) in filter_rows {
            table_states
                .entry(table_name.clone())
                .or_insert_with(|| empty_table_state(table_name))
                .filter_rules
                .push(StoredFilterRule {
                    column_name,
                    mode: filter_mode_from_storage(&mode)?,
                    value,
                });
        }

        Ok(Some(StoredSession {
            mode: app_mode_from_storage(&session_row.0)?,
            focus: pane_focus_from_storage(&session_row.1)?,
            content_view: content_view_from_storage(&session_row.2)?,
            selected_table_name: session_row.3,
            selected_row: usize_from_i64(session_row.4, "selected row")?,
            selected_row_rowid: session_row.5,
            row_offset: usize_from_i64(session_row.6, "row offset")?,
            schema_offset: usize_from_i64(session_row.7, "schema offset")?,
            sql_query: session_row.8,
            sql_cursor: usize_from_i64(session_row.9, "sql cursor")?,
            sql_focus: sql_pane_from_storage(&session_row.10)?,
            sql_history,
            table_states: table_states.into_values().collect(),
        }))
    }

    pub fn save_session(path: &Path, session: &StoredSession) -> Result<()> {
        let normalized = normalize_database_path(path)?;
        Self::save_session_at(&Self::storage_path()?, &normalized, session)
    }

    fn save_session_at(storage_path: &Path, path: &Path, session: &StoredSession) -> Result<()> {
        let normalized = normalize_database_path(path)?;
        let conn = Self::open_at(storage_path)?;
        let encoded = path_to_storage_bytes(&normalized);
        let tx = conn.unchecked_transaction()?;
        let now = unix_timestamp();

        tx.execute(
            "INSERT INTO recent_databases(path, last_opened_at)
             VALUES (?1, ?2)
             ON CONFLICT(path) DO UPDATE SET last_opened_at = MAX(last_opened_at, excluded.last_opened_at)",
            params![encoded.clone(), now],
        )?;
        tx.execute(
            "INSERT INTO app_meta(key, value)
             VALUES ('last_opened_path', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [encoded.clone()],
        )?;
        tx.execute(
            "INSERT INTO sessions(
                 path, mode, focus, content_view, selected_table_name, selected_row,
                 selected_row_rowid, row_offset, schema_offset, sql_query, sql_cursor,
                 sql_focus, updated_at
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
             ON CONFLICT(path) DO UPDATE SET
                 mode = excluded.mode,
                 focus = excluded.focus,
                 content_view = excluded.content_view,
                 selected_table_name = excluded.selected_table_name,
                 selected_row = excluded.selected_row,
                 selected_row_rowid = excluded.selected_row_rowid,
                 row_offset = excluded.row_offset,
                 schema_offset = excluded.schema_offset,
                 sql_query = excluded.sql_query,
                 sql_cursor = excluded.sql_cursor,
                 sql_focus = excluded.sql_focus,
                 updated_at = excluded.updated_at",
            params![
                encoded.clone(),
                app_mode_to_storage(session.mode),
                pane_focus_to_storage(session.focus),
                content_view_to_storage(session.content_view),
                session.selected_table_name.as_deref(),
                session.selected_row as i64,
                session.selected_row_rowid,
                session.row_offset as i64,
                session.schema_offset as i64,
                session.sql_query.as_str(),
                session.sql_cursor as i64,
                sql_pane_to_storage(session.sql_focus),
                now,
            ],
        )?;

        tx.execute("DELETE FROM sql_history WHERE path = ?1", [encoded.clone()])?;
        tx.execute(
            "DELETE FROM hidden_columns WHERE path = ?1",
            [encoded.clone()],
        )?;
        tx.execute("DELETE FROM sort_rules WHERE path = ?1", [encoded.clone()])?;
        tx.execute(
            "DELETE FROM filter_rules WHERE path = ?1",
            [encoded.clone()],
        )?;

        for (position, entry) in session.sql_history.iter().enumerate() {
            tx.execute(
                "INSERT INTO sql_history(path, position, query, summary)
                 VALUES (?1, ?2, ?3, ?4)",
                params![encoded.clone(), position as i64, entry.query, entry.summary],
            )?;
        }

        for table_state in &session.table_states {
            for column_name in &table_state.hidden_columns {
                tx.execute(
                    "INSERT INTO hidden_columns(path, table_name, column_name)
                     VALUES (?1, ?2, ?3)",
                    params![
                        encoded.clone(),
                        table_state.table_name.as_str(),
                        column_name
                    ],
                )?;
            }

            for (position, rule) in table_state.sort_rules.iter().enumerate() {
                tx.execute(
                    "INSERT INTO sort_rules(path, table_name, position, column_name, descending)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        encoded.clone(),
                        table_state.table_name.as_str(),
                        position as i64,
                        rule.column_name.as_str(),
                        i64::from(rule.descending)
                    ],
                )?;
            }

            for (position, rule) in table_state.filter_rules.iter().enumerate() {
                tx.execute(
                    "INSERT INTO filter_rules(path, table_name, position, column_name, mode, value)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        encoded.clone(),
                        table_state.table_name.as_str(),
                        position as i64,
                        rule.column_name.as_str(),
                        filter_mode_to_storage(rule.mode),
                        rule.value.as_str()
                    ],
                )?;
            }
        }

        tx.commit()?;
        Ok(())
    }

    fn open_at(path: &Path) -> Result<Connection> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!(
                    "failed to create app storage directory {}",
                    parent.display()
                )
            })?;
        }

        let conn = Connection::open(path)?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             CREATE TABLE IF NOT EXISTS app_meta(
                 key TEXT PRIMARY KEY,
                 value BLOB NOT NULL
             );
             CREATE TABLE IF NOT EXISTS recent_databases(
                 path BLOB PRIMARY KEY,
                 last_opened_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS sessions(
                 path BLOB PRIMARY KEY,
                 mode TEXT NOT NULL,
                 focus TEXT NOT NULL,
                 content_view TEXT NOT NULL,
                 selected_table_name TEXT,
                 selected_row INTEGER NOT NULL,
                 selected_row_rowid INTEGER,
                 row_offset INTEGER NOT NULL,
                 schema_offset INTEGER NOT NULL,
                 sql_query TEXT NOT NULL,
                 sql_cursor INTEGER NOT NULL,
                 sql_focus TEXT NOT NULL,
                 updated_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS sql_history(
                 path BLOB NOT NULL,
                 position INTEGER NOT NULL,
                 query TEXT NOT NULL,
                 summary TEXT NOT NULL,
                 PRIMARY KEY(path, position)
             );
             CREATE TABLE IF NOT EXISTS hidden_columns(
                 path BLOB NOT NULL,
                 table_name TEXT NOT NULL,
                 column_name TEXT NOT NULL,
                 PRIMARY KEY(path, table_name, column_name)
             );
             CREATE TABLE IF NOT EXISTS sort_rules(
                 path BLOB NOT NULL,
                 table_name TEXT NOT NULL,
                 position INTEGER NOT NULL,
                 column_name TEXT NOT NULL,
                 descending INTEGER NOT NULL,
                 PRIMARY KEY(path, table_name, position)
             );
             CREATE TABLE IF NOT EXISTS filter_rules(
                 path BLOB NOT NULL,
                 table_name TEXT NOT NULL,
                 position INTEGER NOT NULL,
                 column_name TEXT NOT NULL,
                 mode TEXT NOT NULL,
                 value TEXT NOT NULL,
                 PRIMARY KEY(path, table_name, position)
             );",
        )?;
        Ok(conn)
    }

    fn storage_path() -> Result<PathBuf> {
        let base = if cfg!(windows) {
            env::var_os("APPDATA").map(PathBuf::from).or_else(|| {
                env::var_os("USERPROFILE")
                    .map(PathBuf::from)
                    .map(|path| path.join("AppData\\Roaming"))
            })
        } else {
            env::var_os("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .or_else(|| {
                    env::var_os("HOME")
                        .map(PathBuf::from)
                        .map(|path| path.join(".config"))
                })
        }
        .context("unable to determine config directory for app storage")?;

        Ok(base.join("squid").join("state.db"))
    }
}

pub(super) fn normalize_database_path(path: &Path) -> Result<PathBuf> {
    if let Some(uri_path) = normalize_sqlite_uri_path(path)? {
        Ok(uri_path)
    } else if preserves_sqlite_special_name(path) {
        Ok(path.to_path_buf())
    } else if path.is_absolute() {
        Ok(normalize_local_path(path))
    } else {
        Ok(normalize_local_path(
            &env::current_dir()
                .context("failed to resolve current directory")?
                .join(path),
        ))
    }
}

fn normalize_sqlite_uri_path(path: &Path) -> Result<Option<PathBuf>> {
    let Some(raw) = path.to_str() else {
        return Ok(None);
    };
    if !raw.starts_with("file:") {
        return Ok(None);
    }

    let (filename, suffix) = split_sqlite_uri(raw);
    let uri_path = &filename["file:".len()..];
    if uri_path.is_empty()
        || uri_path.starts_with(':')
        || uri_path.starts_with('/')
        || uri_path.starts_with('\\')
        || uri_path.starts_with("//")
        || Path::new(uri_path).is_absolute()
    {
        return Ok(Some(path.to_path_buf()));
    }

    let absolute = normalize_local_path(
        &env::current_dir()
            .context("failed to resolve current directory")?
            .join(uri_path),
    );
    Ok(Some(PathBuf::from(format!(
        "file:{}{}",
        path_to_sqlite_uri_path(&absolute),
        suffix
    ))))
}

fn split_sqlite_uri(raw: &str) -> (&str, &str) {
    let suffix_start = raw.find(['?', '#']).unwrap_or(raw.len());
    (&raw[..suffix_start], &raw[suffix_start..])
}

fn path_to_sqlite_uri_path(path: &Path) -> String {
    let mut normalized = path.to_string_lossy().replace('\\', "/");
    if cfg!(windows) && normalized.as_bytes().get(1) == Some(&b':') {
        normalized.insert(0, '/');
    }
    normalized
}

pub(super) fn recent_path_is_available(path: &Path) -> bool {
    if let Some(local_path) = sqlite_uri_local_path(path) {
        return local_path.is_file();
    }

    if preserves_sqlite_special_name(path) {
        return true;
    }

    path.is_file()
}

#[cfg(test)]
fn recent_paths_match(left: &Path, right: &Path) -> bool {
    match (recent_local_identity(left), recent_local_identity(right)) {
        (Some(left), Some(right)) => left == right,
        _ => left == right,
    }
}

#[cfg(test)]
fn recent_local_identity(path: &Path) -> Option<PathBuf> {
    if let Some(local_path) = sqlite_uri_local_path(path) {
        return Some(local_path);
    }

    if preserves_sqlite_special_name(path) {
        return None;
    }

    if path.to_str().is_some_and(|raw| raw.starts_with("file:")) {
        return None;
    }

    Some(normalize_local_path(path))
}

fn sqlite_uri_local_path(path: &Path) -> Option<PathBuf> {
    let raw = path.to_str()?;
    if !raw.starts_with("file:") {
        return None;
    }

    let (filename, _) = split_sqlite_uri(raw);
    let uri_path = sqlite_uri_local_path_part(&filename["file:".len()..])?;
    if uri_path.is_empty() || uri_path.starts_with(':') {
        return None;
    }

    let decoded_path = percent_decode(&uri_path)?;
    Some(normalize_local_path(&sqlite_uri_path_to_local_path(
        &decoded_path,
    )))
}

fn sqlite_uri_local_path_part(uri_path: &str) -> Option<String> {
    if let Some(authority_and_path) = uri_path.strip_prefix("//") {
        let (authority, path) = match authority_and_path.split_once('/') {
            Some((authority, path)) => (authority, format!("/{path}")),
            None => (authority_and_path, "/".to_string()),
        };
        if authority.is_empty() || authority.eq_ignore_ascii_case("localhost") {
            Some(path)
        } else {
            None
        }
    } else {
        Some(uri_path.to_string())
    }
}

fn normalize_local_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();

    for component in path.components() {
        match component {
            std::path::Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            std::path::Component::RootDir => normalized.push(component.as_os_str()),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if !normalized.pop() && !path.is_absolute() {
                    normalized.push(component.as_os_str());
                }
            }
            std::path::Component::Normal(part) => normalized.push(part),
        }
    }

    normalized
}

fn sqlite_uri_path_to_local_path(uri_path: &str) -> PathBuf {
    if cfg!(windows)
        && uri_path.starts_with('/')
        && uri_path.as_bytes().get(2) == Some(&b':')
        && uri_path.as_bytes().get(3) == Some(&b'/')
    {
        PathBuf::from(&uri_path[1..])
    } else {
        PathBuf::from(uri_path)
    }
}

fn percent_decode(value: &str) -> Option<String> {
    let mut decoded = Vec::with_capacity(value.len());
    let bytes = value.as_bytes();
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len() {
                return None;
            }
            let high = decode_hex(bytes[index + 1])?;
            let low = decode_hex(bytes[index + 2])?;
            decoded.push((high << 4) | low);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }

    String::from_utf8(decoded).ok()
}

#[cfg(unix)]
fn path_to_storage_bytes(path: &Path) -> Vec<u8> {
    path.as_os_str().as_bytes().to_vec()
}

#[cfg(unix)]
fn path_from_storage_bytes(bytes: &[u8]) -> Result<PathBuf> {
    Ok(PathBuf::from(std::ffi::OsString::from_vec(bytes.to_vec())))
}

#[cfg(windows)]
fn path_to_storage_bytes(path: &Path) -> Vec<u8> {
    path.as_os_str()
        .encode_wide()
        .flat_map(|unit| unit.to_le_bytes())
        .collect()
}

#[cfg(windows)]
fn path_from_storage_bytes(bytes: &[u8]) -> Result<PathBuf> {
    if !bytes.len().is_multiple_of(2) {
        anyhow::bail!("stored path has an odd number of UTF-16 bytes");
    }

    let wide = bytes
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect::<Vec<_>>();
    Ok(PathBuf::from(std::ffi::OsString::from_wide(&wide)))
}

fn decode_hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn preserves_sqlite_special_name(path: &Path) -> bool {
    match path.to_str() {
        Some(":memory:") => true,
        None => false,
        Some(_) => false,
    }
}

fn unix_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

fn empty_table_state(table_name: String) -> StoredTableState {
    StoredTableState {
        table_name,
        hidden_columns: Vec::new(),
        sort_rules: Vec::new(),
        filter_rules: Vec::new(),
    }
}

fn usize_from_i64(value: i64, label: &str) -> Result<usize> {
    usize::try_from(value).with_context(|| format!("{label} overflowed usize"))
}

fn app_mode_to_storage(value: AppMode) -> &'static str {
    match value {
        AppMode::Home => "home",
        AppMode::Browse => "browse",
        AppMode::Sql => "sql",
    }
}

fn app_mode_from_storage(value: &str) -> Result<AppMode> {
    match value {
        "home" => Ok(AppMode::Home),
        "browse" => Ok(AppMode::Browse),
        "sql" => Ok(AppMode::Sql),
        _ => anyhow::bail!("unknown stored app mode {value}"),
    }
}

fn pane_focus_to_storage(value: PaneFocus) -> &'static str {
    match value {
        PaneFocus::Tables => "tables",
        PaneFocus::Content => "content",
    }
}

fn pane_focus_from_storage(value: &str) -> Result<PaneFocus> {
    match value {
        "tables" => Ok(PaneFocus::Tables),
        "content" => Ok(PaneFocus::Content),
        _ => anyhow::bail!("unknown stored pane focus {value}"),
    }
}

fn content_view_to_storage(value: ContentView) -> &'static str {
    match value {
        ContentView::Rows => "rows",
        ContentView::Schema => "schema",
    }
}

fn content_view_from_storage(value: &str) -> Result<ContentView> {
    match value {
        "rows" => Ok(ContentView::Rows),
        "schema" => Ok(ContentView::Schema),
        _ => anyhow::bail!("unknown stored content view {value}"),
    }
}

fn sql_pane_to_storage(value: SqlPane) -> &'static str {
    match value {
        SqlPane::Editor => "editor",
        SqlPane::History => "history",
        SqlPane::Results => "results",
    }
}

fn sql_pane_from_storage(value: &str) -> Result<SqlPane> {
    match value {
        "editor" => Ok(SqlPane::Editor),
        "history" => Ok(SqlPane::History),
        "results" => Ok(SqlPane::Results),
        _ => anyhow::bail!("unknown stored sql pane {value}"),
    }
}

fn filter_mode_to_storage(value: FilterMode) -> &'static str {
    match value {
        FilterMode::Contains => "contains",
        FilterMode::Equals => "equals",
        FilterMode::StartsWith => "starts_with",
        FilterMode::GreaterThan => "greater_than",
        FilterMode::LessThan => "less_than",
        FilterMode::IsTrue => "is_true",
        FilterMode::IsFalse => "is_false",
    }
}

fn filter_mode_from_storage(value: &str) -> Result<FilterMode> {
    match value {
        "contains" => Ok(FilterMode::Contains),
        "equals" => Ok(FilterMode::Equals),
        "starts_with" => Ok(FilterMode::StartsWith),
        "greater_than" => Ok(FilterMode::GreaterThan),
        "less_than" => Ok(FilterMode::LessThan),
        "is_true" => Ok(FilterMode::IsTrue),
        "is_false" => Ok(FilterMode::IsFalse),
        _ => anyhow::bail!("unknown stored filter mode {value}"),
    }
}

#[cfg(test)]
mod tests;
