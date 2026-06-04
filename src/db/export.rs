use anyhow::Result;

use super::{Database, FilterClause, SortClause};

impl Database {
    pub fn export_table_rows(
        &self,
        table_name: &str,
        visible_columns: &[String],
        sort_clauses: &[SortClause],
        filter_clauses: &[FilterClause],
    ) -> Result<(Vec<String>, Vec<Vec<String>>)> {
        let total_rows = self.count_table_rows(table_name, filter_clauses)?;
        if total_rows == 0 {
            return Ok((visible_columns.to_vec(), Vec::new()));
        }

        let mut columns = Vec::new();
        let mut rows = Vec::with_capacity(total_rows);
        let page_size = 500;
        let mut offset = 0;

        while offset < total_rows {
            let page = self.preview_table_page(
                table_name,
                visible_columns,
                sort_clauses,
                filter_clauses,
                page_size,
                offset,
                total_rows,
            )?;
            if columns.is_empty() {
                columns = page.columns;
            }
            let fetched = page.rows.len();
            rows.extend(page.rows);
            if fetched == 0 {
                break;
            }
            offset += fetched;
        }

        Ok((columns, rows))
    }
}

pub fn serialize_csv(columns: &[String], rows: &[Vec<String>]) -> String {
    let mut output = columns
        .iter()
        .map(|column| csv_escape(column))
        .collect::<Vec<_>>()
        .join(",");
    output.push('\n');

    for row in rows {
        let line = row
            .iter()
            .map(|value| csv_escape(value))
            .collect::<Vec<_>>()
            .join(",");
        output.push_str(&line);
        output.push('\n');
    }

    output
}

pub fn default_export_path(table_label: &str) -> std::path::PathBuf {
    let safe_name: String = table_label
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    let file_stem = if safe_name.is_empty() {
        "export".to_string()
    } else {
        safe_name
    };
    std::path::PathBuf::from(format!("{file_stem}.csv"))
}

fn csv_escape(value: &str) -> String {
    if needs_csv_quoting(value) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn needs_csv_quoting(value: &str) -> bool {
    value.contains(['"', ',', '\n', '\r'])
}

#[cfg(test)]
mod tests {
    use super::{default_export_path, serialize_csv};

    #[test]
    fn serialize_csv_quotes_fields_with_commas_and_newlines() {
        let csv = serialize_csv(
            &["id".to_string(), "name".to_string()],
            &[vec!["1".to_string(), "hello, \"world\"\nline2".to_string()]],
        );
        assert_eq!(csv, "id,name\n1,\"hello, \"\"world\"\"\nline2\"\n");
    }

    #[test]
    fn default_export_path_sanitizes_table_label() {
        assert_eq!(
            default_export_path("main.users"),
            std::path::PathBuf::from("main_users.csv")
        );
    }

    #[test]
    fn serialize_csv_writes_header_and_rows() {
        let csv = serialize_csv(
            &["a".to_string()],
            &[vec!["1".to_string()], vec!["2".to_string()]],
        );
        assert_eq!(csv, "a\n1\n2\n");
    }
}
