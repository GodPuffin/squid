use rusqlite::types::ValueRef;

const BLOB_DETAIL_MAX_BYTES: usize = 4096;

pub(crate) fn format_value(value: ValueRef<'_>) -> String {
    match value {
        ValueRef::Null => "NULL".to_string(),
        ValueRef::Integer(v) => v.to_string(),
        ValueRef::Real(v) => v.to_string(),
        ValueRef::Text(v) => String::from_utf8_lossy(v).into_owned(),
        ValueRef::Blob(v) => format!("<{} bytes>", v.len()),
    }
}

pub(crate) fn format_detail_value(value: ValueRef<'_>) -> String {
    match value {
        ValueRef::Blob(bytes) => format_blob_detail(bytes),
        other => format_value(other),
    }
}

pub(crate) fn format_blob_detail(bytes: &[u8]) -> String {
    let truncated = bytes.len() > BLOB_DETAIL_MAX_BYTES;
    let display = if truncated {
        &bytes[..BLOB_DETAIL_MAX_BYTES]
    } else {
        bytes
    };

    let mut lines = vec![format!("<{} bytes>", bytes.len())];
    if truncated {
        lines.push(format!(
            "(showing first {BLOB_DETAIL_MAX_BYTES} bytes; scroll to view)"
        ));
    }
    lines.push(String::new());
    lines.push("Hex:".to_string());
    lines.extend(format_blob_hex_lines(display));

    if let Some(preview) = format_blob_utf8_preview(display) {
        lines.push(String::new());
        lines.push("UTF-8:".to_string());
        lines.push(preview);
    }

    lines.join("\n")
}

pub(crate) fn format_blob_hex_lines(bytes: &[u8]) -> Vec<String> {
    if bytes.is_empty() {
        return vec!["<empty>".to_string()];
    }

    bytes
        .chunks(16)
        .enumerate()
        .map(|(index, chunk)| {
            let offset = index * 16;
            let hex = chunk
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<Vec<_>>()
                .join(" ");
            format!("{offset:04x}  {hex}")
        })
        .collect()
}

pub(crate) fn format_blob_utf8_preview(bytes: &[u8]) -> Option<String> {
    if bytes.is_empty() {
        return None;
    }

    let text = String::from_utf8_lossy(bytes);
    if text.chars().any(|ch| ch == '\u{FFFD}') {
        return None;
    }

    let preview: String = text
        .chars()
        .map(|ch| {
            if ch.is_control() && ch != '\n' && ch != '\t' {
                '.'
            } else {
                ch
            }
        })
        .collect();

    if preview.trim().is_empty() {
        None
    } else {
        Some(preview)
    }
}

#[cfg(test)]
mod tests;
