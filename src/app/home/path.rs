use std::env;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

pub(crate) fn normalize_database_path(path: &Path) -> Result<PathBuf> {
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

pub(crate) fn path_to_sqlite_uri_path(path: &Path) -> String {
    let mut normalized = path.to_string_lossy().replace('\\', "/");
    if cfg!(windows) && normalized.as_bytes().get(1) == Some(&b':') {
        normalized.insert(0, '/');
    }
    normalized
}

pub(crate) fn recent_path_is_available(path: &Path) -> bool {
    if let Some(local_path) = sqlite_uri_local_path(path) {
        return local_path.is_file();
    }

    if preserves_sqlite_special_name(path) {
        return true;
    }

    path.is_file()
}

#[cfg(test)]
pub(crate) fn recent_paths_match(left: &Path, right: &Path) -> bool {
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
