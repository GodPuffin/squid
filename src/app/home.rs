use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecentItem {
    pub path: PathBuf,
    pub available: bool,
}

pub struct RecentStore;

impl RecentStore {
    const MAX_ITEMS: usize = 10;

    pub fn load() -> Result<Vec<RecentItem>> {
        let paths = Self::load_from_path(&Self::storage_path()?)?;
        Ok(Self::to_items(paths))
    }

    pub fn record(path: &Path) -> Result<Vec<RecentItem>> {
        let absolute = normalize_database_path(path)?;
        let storage_path = Self::storage_path()?;
        let mut paths = Self::load_from_path(&storage_path)?;
        paths.retain(|existing| existing != &absolute);
        paths.insert(0, absolute);
        paths.truncate(Self::MAX_ITEMS);
        Self::save_to_path(&storage_path, &paths)?;
        Ok(Self::to_items(paths))
    }

    pub fn remove(path: &Path) -> Result<Vec<RecentItem>> {
        let storage_path = Self::storage_path()?;
        let mut paths = Self::load_from_path(&storage_path)?;
        paths.retain(|existing| existing != path);
        Self::save_to_path(&storage_path, &paths)?;
        Ok(Self::to_items(paths))
    }

    fn to_items(paths: Vec<PathBuf>) -> Vec<RecentItem> {
        paths
            .into_iter()
            .map(|path| RecentItem {
                available: path.is_file(),
                path,
            })
            .collect()
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
        .context("unable to determine config directory for recent databases")?;

        Ok(base.join("squid").join("recent.txt"))
    }

    fn load_from_path(path: &Path) -> Result<Vec<PathBuf>> {
        let Ok(contents) = fs::read_to_string(path) else {
            return Ok(Vec::new());
        };

        Ok(contents
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(PathBuf::from)
            .collect())
    }

    fn save_to_path(path: &Path, paths: &[PathBuf]) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!(
                    "failed to create recent database directory {}",
                    parent.display()
                )
            })?;
        }

        let mut body = paths
            .iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("\n");
        if !body.is_empty() {
            body.push('\n');
        }

        fs::write(path, body)
            .with_context(|| format!("failed to write recent database list {}", path.display()))
    }
}

pub(super) fn normalize_database_path(path: &Path) -> Result<PathBuf> {
    if preserves_sqlite_filename(path) || path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(env::current_dir()
            .context("failed to resolve current directory")?
            .join(path))
    }
}

fn preserves_sqlite_filename(path: &Path) -> bool {
    match path.to_str() {
        Some(":memory:") => true,
        Some(filename) => filename.starts_with("file:"),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{RecentStore, normalize_database_path};

    #[test]
    fn load_from_path_ignores_blank_lines() {
        let path = unique_test_path("load");
        std::fs::write(&path, "\nC:\\db1.sqlite\n\nC:\\db2.sqlite\n").unwrap();

        let paths = RecentStore::load_from_path(&path).unwrap();

        assert_eq!(paths.len(), 2);
        cleanup(&path);
    }

    #[test]
    fn save_and_remove_preserve_recent_order() {
        let path = unique_test_path("save");
        let entries = vec![
            std::path::PathBuf::from("C:\\db1.sqlite"),
            std::path::PathBuf::from("C:\\db2.sqlite"),
        ];

        RecentStore::save_to_path(&path, &entries).unwrap();
        let loaded = RecentStore::load_from_path(&path).unwrap();
        assert_eq!(loaded, entries);

        let filtered = loaded
            .into_iter()
            .filter(|entry| entry != &std::path::PathBuf::from("C:\\db1.sqlite"))
            .collect::<Vec<_>>();
        RecentStore::save_to_path(&path, &filtered).unwrap();

        let after = RecentStore::load_from_path(&path).unwrap();
        assert_eq!(after, vec![std::path::PathBuf::from("C:\\db2.sqlite")]);
        cleanup(&path);
    }

    #[test]
    fn record_logic_moves_existing_to_front_and_trims() {
        let mut entries = (0..12)
            .map(|index| std::path::PathBuf::from(format!("C:\\db{index}.sqlite")))
            .collect::<Vec<_>>();
        let target = std::path::PathBuf::from("C:\\db5.sqlite");

        entries.retain(|entry| entry != &target);
        entries.insert(0, target.clone());
        entries.truncate(10);

        assert_eq!(entries.first(), Some(&target));
        assert_eq!(entries.len(), 10);
    }

    #[test]
    fn normalize_database_path_preserves_memory_databases() {
        let path = std::path::Path::new(":memory:");

        assert_eq!(normalize_database_path(path).unwrap(), path);
    }

    #[test]
    fn normalize_database_path_preserves_sqlite_uri_filenames() {
        let path = std::path::Path::new("file:/tmp/app.db?mode=ro");

        assert_eq!(normalize_database_path(path).unwrap(), path);
    }

    #[test]
    fn normalize_database_path_absolutizes_regular_relative_paths() {
        let path = std::path::Path::new("sakila.db");
        let normalized = normalize_database_path(path).unwrap();

        assert!(normalized.is_absolute());
        assert!(normalized.ends_with(path));
    }

    fn unique_test_path(label: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("squid-{label}-{nanos}.txt"))
    }

    fn cleanup(path: &std::path::Path) {
        let _ = std::fs::remove_file(path);
    }
}
