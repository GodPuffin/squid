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
                available: recent_path_is_available(&path),
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

fn recent_path_is_available(path: &Path) -> bool {
    if let Some(local_path) = sqlite_uri_local_path(path) {
        return local_path.is_file();
    }

    if preserves_sqlite_special_name(path) {
        return true;
    }

    path.is_file()
}

fn sqlite_uri_local_path(path: &Path) -> Option<PathBuf> {
    let raw = path.to_str()?;
    if !raw.starts_with("file:") {
        return None;
    }

    let (filename, _) = split_sqlite_uri(raw);
    let uri_path = &filename["file:".len()..];
    if uri_path.is_empty() || uri_path.starts_with(':') {
        return None;
    }
    if uri_path.starts_with("//") {
        return None;
    }

    Some(normalize_local_path(&sqlite_uri_path_to_local_path(
        uri_path,
    )))
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

fn preserves_sqlite_special_name(path: &Path) -> bool {
    match path.to_str() {
        Some(":memory:") => true,
        None => false,
        Some(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        RecentStore, normalize_database_path, path_to_sqlite_uri_path, recent_path_is_available,
    };

    #[test]
    fn load_from_path_ignores_blank_lines() {
        let path = unique_test_path("load");
        std::fs::write(&path, "\nC:\\db1.sqlite\n\nC:\\db2.sqlite\n").unwrap();

        let paths = RecentStore::load_from_path(&path).unwrap();

        assert_eq!(paths.len(), 2);
        cleanup(&path);
    }

    #[test]
    fn load_from_path_preserves_surrounding_whitespace() {
        let path = unique_test_path("load-whitespace");
        std::fs::write(&path, " report.db\nreport.db \n").unwrap();

        let paths = RecentStore::load_from_path(&path).unwrap();

        assert_eq!(
            paths,
            vec![
                std::path::PathBuf::from(" report.db"),
                std::path::PathBuf::from("report.db "),
            ]
        );
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
    fn normalize_database_path_absolutizes_relative_sqlite_uri_filenames() {
        let path = std::path::Path::new("file:./fixtures/app.db?mode=ro");
        let expected = std::env::current_dir().unwrap().join("./fixtures/app.db");

        assert_eq!(
            normalize_database_path(path).unwrap(),
            std::path::PathBuf::from(format!(
                "file:{}?mode=ro",
                path_to_sqlite_uri_path(&expected)
            ))
        );
    }

    #[test]
    fn normalize_database_path_absolutizes_regular_relative_paths() {
        let path = std::path::Path::new("sakila.db");
        let normalized = normalize_database_path(path).unwrap();

        assert!(normalized.is_absolute());
        assert!(normalized.ends_with(path));
    }

    #[test]
    fn normalize_database_path_collapses_lexical_aliases() {
        let canonical = normalize_database_path(std::path::Path::new("sakila.db")).unwrap();
        let dotted = normalize_database_path(std::path::Path::new("./sakila.db")).unwrap();
        let parent = normalize_database_path(std::path::Path::new("sub/../sakila.db")).unwrap();

        assert_eq!(canonical, dotted);
        assert_eq!(canonical, parent);
    }

    #[test]
    fn recent_path_is_available_for_sqlite_file_uris() {
        let path = std::env::temp_dir().join(format!(
            "squid-available-{}.db",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&path, b"sqlite").unwrap();

        let uri =
            std::path::PathBuf::from(format!("file:{}?mode=ro", path_to_sqlite_uri_path(&path)));
        assert!(recent_path_is_available(&uri));

        cleanup(&path);
    }

    #[test]
    fn normalize_database_path_preserves_memory_file_uris() {
        let path = std::path::Path::new("file::memory:?cache=shared");

        assert_eq!(normalize_database_path(path).unwrap(), path);
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
