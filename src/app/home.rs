use std::env;
use std::fs;
use std::io::ErrorKind;
#[cfg(unix)]
use std::os::unix::ffi::{OsStrExt, OsStringExt};
#[cfg(windows)]
use std::os::windows::ffi::{OsStrExt, OsStringExt};
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
    const STORAGE_MAGIC: &'static [u8] = b"SQUIDREC1";

    pub fn load() -> Result<Vec<RecentItem>> {
        let paths = Self::load_from_path(&Self::storage_path()?)?;
        Ok(Self::to_items(paths))
    }

    pub fn record(path: &Path) -> Result<Vec<RecentItem>> {
        let absolute = normalize_database_path(path)?;
        let storage_path = Self::storage_path()?;
        let mut paths = Self::load_from_path(&storage_path)?;
        paths.retain(|existing| !recent_paths_match(existing, &absolute));
        paths.insert(0, absolute);
        paths.truncate(Self::MAX_ITEMS);
        Self::save_to_path(&storage_path, &paths)?;
        Ok(Self::to_items(paths))
    }

    pub fn remove(path: &Path) -> Result<Vec<RecentItem>> {
        let storage_path = Self::storage_path()?;
        let mut paths = Self::load_from_path(&storage_path)?;
        paths.retain(|existing| !recent_paths_match(existing, path));
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
        let contents = match fs::read(path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("failed to read recent database list {}", path.display())
                });
            }
        };

        if contents.starts_with(Self::STORAGE_MAGIC) {
            return Self::load_binary_paths(&contents[Self::STORAGE_MAGIC.len()..]).with_context(
                || format!("failed to read recent database list {}", path.display()),
            );
        }

        Self::load_legacy_text_paths(&contents)
            .with_context(|| format!("failed to read recent database list {}", path.display()))
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

        let mut body = Vec::new();
        body.extend_from_slice(Self::STORAGE_MAGIC);
        for path in paths {
            let encoded = path_to_storage_bytes(path);
            body.extend_from_slice(&(encoded.len() as u32).to_le_bytes());
            body.extend_from_slice(&encoded);
        }

        fs::write(path, body)
            .with_context(|| format!("failed to write recent database list {}", path.display()))
    }

    fn load_binary_paths(mut bytes: &[u8]) -> Result<Vec<PathBuf>> {
        let mut paths = Vec::new();
        while !bytes.is_empty() {
            if bytes.len() < 4 {
                anyhow::bail!("truncated recent database entry length");
            }

            let length = u32::from_le_bytes(bytes[..4].try_into().unwrap()) as usize;
            bytes = &bytes[4..];
            if bytes.len() < length {
                anyhow::bail!("truncated recent database entry payload");
            }

            paths.push(path_from_storage_bytes(&bytes[..length])?);
            bytes = &bytes[length..];
        }

        Ok(paths)
    }

    fn load_legacy_text_paths(bytes: &[u8]) -> Result<Vec<PathBuf>> {
        let contents =
            String::from_utf8(bytes.to_vec()).context("recent database list is not valid UTF-8")?;
        Ok(contents
            .lines()
            .filter(|line| !line.is_empty())
            .map(PathBuf::from)
            .collect())
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

fn recent_paths_match(left: &Path, right: &Path) -> bool {
    match (recent_local_identity(left), recent_local_identity(right)) {
        (Some(left), Some(right)) => left == right,
        _ => left == right,
    }
}

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
    if uri_path.starts_with("//") {
        let authority_and_path = &uri_path["//".len()..];
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
    if bytes.len() % 2 != 0 {
        anyhow::bail!("recent database entry has an odd number of UTF-16 bytes");
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

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use std::os::unix::ffi::OsStringExt;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        RecentStore, normalize_database_path, path_to_sqlite_uri_path, recent_path_is_available,
        recent_paths_match,
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
    fn load_from_path_reports_non_not_found_errors() {
        let path = unique_test_path("load-dir");
        std::fs::create_dir_all(&path).unwrap();

        let error = RecentStore::load_from_path(&path).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("failed to read recent database list")
        );
        let _ = std::fs::remove_dir(&path);
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
    fn save_and_load_preserve_newlines_in_paths() {
        let path = unique_test_path("save-newline");
        let entries = vec![std::path::PathBuf::from("report\n2026.db")];

        RecentStore::save_to_path(&path, &entries).unwrap();

        let loaded = RecentStore::load_from_path(&path).unwrap();
        assert_eq!(loaded, entries);
        cleanup(&path);
    }

    #[cfg(unix)]
    #[test]
    fn save_and_load_preserve_non_utf8_paths() {
        let path = unique_test_path("save-nonutf8");
        let entries = vec![std::path::PathBuf::from(std::ffi::OsString::from_vec(
            vec![b'r', b'e', b'p', b'o', b'r', b't', 0xff, b'.', b'd', b'b'],
        ))];

        RecentStore::save_to_path(&path, &entries).unwrap();

        let loaded = RecentStore::load_from_path(&path).unwrap();
        assert_eq!(loaded, entries);
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
    fn recent_paths_match_plain_paths_and_file_uri_aliases() {
        let path = std::env::temp_dir().join("squid-recent-match.db");
        let raw_path = path.clone();
        let file_uri =
            std::path::PathBuf::from(format!("file:{}?mode=ro", path_to_sqlite_uri_path(&path)));
        let localhost_uri = std::path::PathBuf::from(format!(
            "file://localhost{}?mode=ro",
            path_to_sqlite_uri_path(&path)
        ));

        assert!(recent_paths_match(&raw_path, &file_uri));
        assert!(recent_paths_match(&file_uri, &localhost_uri));
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
    fn recent_path_is_available_for_localhost_file_uris() {
        let path = std::env::temp_dir().join(format!(
            "squid-localhost-{}.db",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&path, b"sqlite").unwrap();

        let uri = std::path::PathBuf::from(format!(
            "file://localhost{}?mode=ro",
            path_to_sqlite_uri_path(&path)
        ));
        assert!(recent_path_is_available(&uri));

        cleanup(&path);
    }

    #[test]
    fn recent_path_is_available_for_percent_encoded_file_uris() {
        let path = std::env::temp_dir().join(format!(
            "squid my db {}.sqlite",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&path, b"sqlite").unwrap();

        let encoded_path = path_to_sqlite_uri_path(&path).replace(' ', "%20");
        let uri = std::path::PathBuf::from(format!("file:{encoded_path}?mode=ro"));
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
