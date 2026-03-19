use std::env;
use std::fs;
use std::io::ErrorKind;
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
        let contents = match fs::read_to_string(path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("failed to read recent database list {}", path.display())
                });
            }
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
#[path = "../testing/app/home.rs"]
mod tests;
