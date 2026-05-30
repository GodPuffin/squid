use std::env;
use std::fs;
#[cfg(unix)]
use std::os::unix::ffi::{OsStrExt, OsStringExt};
#[cfg(windows)]
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
#[cfg(test)]
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};

use crate::app::{AppMode, ContentView, PaneFocus, SqlHistoryEntry, SqlPane};
use crate::app::{AppSettings, DefaultBrowseView};
use crate::db::FilterMode;

use super::{normalize_database_path, recent_path_is_available};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecentItem {
    pub path: PathBuf,
    pub available: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct StoredSortRule {
    pub column_name: String,
    pub descending: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct StoredFilterRule {
    pub column_name: String,
    pub mode: FilterMode,
    pub value: String,
}

#[derive(Clone, Debug)]
pub(crate) struct StoredTableState {
    pub table_name: String,
    pub hidden_columns: Vec<String>,
    pub sort_rules: Vec<StoredSortRule>,
    pub filter_rules: Vec<StoredFilterRule>,
}

#[derive(Clone, Debug)]
pub(crate) struct StoredSession {
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
    pub fn load() -> Result<Vec<RecentItem>> {
        let limit = crate::app::AppSettings::load()
            .map(|settings| settings.recent_limit)
            .unwrap_or(10);
        AppStorage::load_recent(limit)
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

pub(crate) struct AppStorage;

impl AppStorage {
    pub fn load_recent(limit: usize) -> Result<Vec<RecentItem>> {
        Self::load_recent_from_path(&Self::storage_path()?, limit)
    }

    pub(crate) fn load_recent_from_path(
        storage_path: &Path,
        limit: usize,
    ) -> Result<Vec<RecentItem>> {
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

    pub(crate) fn record_recent_at(storage_path: &Path, path: &Path) -> Result<()> {
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

    pub(crate) fn remove_recent_at(storage_path: &Path, path: &Path) -> Result<()> {
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

    pub fn last_opened_path() -> Result<Option<PathBuf>> {
        Self::last_opened_path_at(&Self::storage_path()?)
    }

    pub(crate) fn last_opened_path_at(storage_path: &Path) -> Result<Option<PathBuf>> {
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

    pub fn load_settings() -> Result<AppSettings> {
        Self::load_settings_at(&Self::storage_path()?)
    }

    pub fn save_settings(settings: &AppSettings) -> Result<()> {
        Self::save_settings_at(&Self::storage_path()?, settings)
    }

    pub(crate) fn load_settings_at(storage_path: &Path) -> Result<AppSettings> {
        let conn = Self::open_at(storage_path)?;
        let mut settings = AppSettings::default();

        let mut stmt = conn.prepare("SELECT key, value FROM app_settings")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;

        for row in rows {
            let (key, value) = row?;
            match key.as_str() {
                "mouse_enabled" => settings.mouse_enabled = parse_settings_bool(&value)?,
                "restore_session_on_open" => {
                    settings.restore_session_on_open = parse_settings_bool(&value)?;
                }
                "auto_open_last_database" => {
                    settings.auto_open_last_database = parse_settings_bool(&value)?;
                }
                "recent_limit" => {
                    settings.recent_limit = parse_settings_usize(&value, "recent_limit")?;
                }
                "sql_result_row_limit" => {
                    settings.sql_result_row_limit =
                        parse_settings_usize(&value, "sql_result_row_limit")?;
                }
                "confirm_before_remove_recent" => {
                    settings.confirm_before_remove_recent = parse_settings_bool(&value)?;
                }
                "default_browse_view" => {
                    settings.default_browse_view = default_browse_view_from_storage(&value)?;
                }
                "sql_history_size" => {
                    settings.sql_history_size = parse_settings_usize(&value, "sql_history_size")?;
                }
                "live_table_search" => settings.live_table_search = parse_settings_bool(&value)?,
                "show_row_numbers" => settings.show_row_numbers = parse_settings_bool(&value)?,
                "cell_preview_max_chars" => {
                    settings.cell_preview_max_chars =
                        parse_settings_usize(&value, "cell_preview_max_chars")?;
                }
                "double_click_interval_ms" => {
                    settings.double_click_interval_ms =
                        parse_settings_u64(&value, "double_click_interval_ms")?;
                }
                "restore_cursor_on_startup" => {
                    settings.restore_cursor_on_startup = parse_settings_bool(&value)?;
                }
                "clear_session_on_quit" => {
                    settings.clear_session_on_quit = parse_settings_bool(&value)?;
                }
                _ => {}
            }
        }

        settings.recent_limit = settings.recent_limit.clamp(1, 100);
        settings.sql_result_row_limit = settings.sql_result_row_limit.clamp(1, 100_000);
        settings.sql_history_size = settings.sql_history_size.clamp(1, 500);
        settings.cell_preview_max_chars = settings.cell_preview_max_chars.clamp(0, 10_000);
        settings.double_click_interval_ms = settings.double_click_interval_ms.clamp(100, 2000);

        Ok(settings)
    }

    pub(crate) fn save_settings_at(storage_path: &Path, settings: &AppSettings) -> Result<()> {
        let conn = Self::open_at(storage_path)?;
        let pairs = [
            (
                "mouse_enabled",
                settings_bool_to_storage(settings.mouse_enabled),
            ),
            (
                "restore_session_on_open",
                settings_bool_to_storage(settings.restore_session_on_open),
            ),
            (
                "auto_open_last_database",
                settings_bool_to_storage(settings.auto_open_last_database),
            ),
            ("recent_limit", settings.recent_limit.to_string()),
            (
                "sql_result_row_limit",
                settings.sql_result_row_limit.to_string(),
            ),
            (
                "confirm_before_remove_recent",
                settings_bool_to_storage(settings.confirm_before_remove_recent),
            ),
            (
                "default_browse_view",
                default_browse_view_to_storage(settings.default_browse_view),
            ),
            ("sql_history_size", settings.sql_history_size.to_string()),
            (
                "live_table_search",
                settings_bool_to_storage(settings.live_table_search),
            ),
            (
                "show_row_numbers",
                settings_bool_to_storage(settings.show_row_numbers),
            ),
            (
                "cell_preview_max_chars",
                settings.cell_preview_max_chars.to_string(),
            ),
            (
                "double_click_interval_ms",
                settings.double_click_interval_ms.to_string(),
            ),
            (
                "restore_cursor_on_startup",
                settings_bool_to_storage(settings.restore_cursor_on_startup),
            ),
            (
                "clear_session_on_quit",
                settings_bool_to_storage(settings.clear_session_on_quit),
            ),
        ];

        let tx = conn.unchecked_transaction()?;
        for (key, value) in pairs {
            tx.execute(
                "INSERT INTO app_settings(key, value)
                 VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                [key, value.as_str()],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn load_session(path: &Path) -> Result<Option<StoredSession>> {
        let normalized = normalize_database_path(path)?;
        Self::load_session_at(&Self::storage_path()?, &normalized)
    }

    pub(crate) fn load_session_at(
        storage_path: &Path,
        path: &Path,
    ) -> Result<Option<StoredSession>> {
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

    pub(crate) fn save_session_at(
        storage_path: &Path,
        path: &Path,
        session: &StoredSession,
    ) -> Result<()> {
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

        let mut conn = Connection::open(path)?;
        migrate_legacy_recent_storage(&mut conn)?;
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
             );
             CREATE TABLE IF NOT EXISTS app_settings(
                 key TEXT PRIMARY KEY,
                 value TEXT NOT NULL
             );",
        )?;
        Ok(conn)
    }

    fn storage_path() -> Result<PathBuf> {
        if let Some(path) = env_path("SQUID_STATE_DB") {
            return Ok(path);
        }
        if let Some(path) = env_path("SQUID_CONFIG_DIR") {
            return Ok(path.join("state.db"));
        }

        #[cfg(test)]
        {
            Ok(test_storage_path())
        }

        #[cfg(not(test))]
        {
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
}

fn migrate_legacy_recent_storage(conn: &mut Connection) -> Result<()> {
    let columns = {
        let mut stmt = conn.prepare("PRAGMA table_info(recent_databases)")?;
        stmt.query_map([], |row| {
            Ok((row.get::<_, String>(1)?, row.get::<_, i64>(5)? != 0))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?
    };

    if columns.is_empty() {
        return Ok(());
    }

    let has_path = columns.iter().any(|(name, _)| name == "path");
    let path_is_primary = columns
        .iter()
        .any(|(name, is_primary)| name == "path" && *is_primary);
    if !has_path || path_is_primary {
        return Ok(());
    }

    let tx = conn.transaction()?;
    tx.execute_batch(
        "DROP TABLE IF EXISTS recent_databases__migrated;
         CREATE TABLE recent_databases__migrated(
             path BLOB PRIMARY KEY,
             last_opened_at INTEGER NOT NULL
         );
         INSERT INTO recent_databases__migrated(path, last_opened_at)
         SELECT path, MAX(last_opened_at)
         FROM recent_databases
         GROUP BY path;
         DROP TABLE recent_databases;
         ALTER TABLE recent_databases__migrated RENAME TO recent_databases;",
    )
    .context("failed to migrate legacy recent database storage")?;
    tx.commit()?;
    Ok(())
}

fn env_path(name: &str) -> Option<PathBuf> {
    let value = env::var_os(name)?;
    if value.is_empty() {
        None
    } else {
        Some(PathBuf::from(value))
    }
}

#[cfg(test)]
pub(crate) fn test_storage_path() -> PathBuf {
    env::temp_dir().join(format!("squid-test-state-{}.db", process::id()))
}

#[cfg(unix)]
pub(crate) fn path_to_storage_bytes(path: &Path) -> Vec<u8> {
    path.as_os_str().as_bytes().to_vec()
}

#[cfg(unix)]
pub(crate) fn path_from_storage_bytes(bytes: &[u8]) -> Result<PathBuf> {
    Ok(PathBuf::from(std::ffi::OsString::from_vec(bytes.to_vec())))
}

#[cfg(windows)]
pub(crate) fn path_to_storage_bytes(path: &Path) -> Vec<u8> {
    path.as_os_str()
        .encode_wide()
        .flat_map(|unit| unit.to_le_bytes())
        .collect()
}

#[cfg(windows)]
pub(crate) fn path_from_storage_bytes(bytes: &[u8]) -> Result<PathBuf> {
    if !bytes.len().is_multiple_of(2) {
        anyhow::bail!("stored path has an odd number of UTF-16 bytes");
    }

    let wide = bytes
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect::<Vec<_>>();
    Ok(PathBuf::from(std::ffi::OsString::from_wide(&wide)))
}

fn parse_settings_bool(value: &str) -> Result<bool> {
    match value {
        "1" | "true" | "on" => Ok(true),
        "0" | "false" | "off" => Ok(false),
        other => anyhow::bail!("invalid boolean setting value: {other}"),
    }
}

fn parse_settings_usize(value: &str, name: &str) -> Result<usize> {
    value
        .parse::<usize>()
        .with_context(|| format!("invalid {name} setting value: {value}"))
}

fn parse_settings_u64(value: &str, name: &str) -> Result<u64> {
    value
        .parse::<u64>()
        .with_context(|| format!("invalid {name} setting value: {value}"))
}

fn default_browse_view_from_storage(value: &str) -> Result<DefaultBrowseView> {
    match value {
        "rows" => Ok(DefaultBrowseView::Rows),
        "schema" => Ok(DefaultBrowseView::Schema),
        other => anyhow::bail!("invalid default browse view setting value: {other}"),
    }
}

fn default_browse_view_to_storage(value: DefaultBrowseView) -> String {
    value.label().to_string()
}

fn settings_bool_to_storage(value: bool) -> String {
    if value { "1" } else { "0" }.to_string()
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
