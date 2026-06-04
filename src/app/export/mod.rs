use std::path::Path;

use anyhow::Result;

use crate::db::export::{default_export_path, serialize_csv};

use super::{App, ContentView};

impl App {
    pub(in crate::app) fn export_current_view(&mut self) -> Result<()> {
        if self.is_home() || self.content_view != ContentView::Rows {
            return Ok(());
        }

        let Some(table_name) = self.selected_table_name().map(str::to_owned) else {
            return Ok(());
        };

        let visible_columns = self.visible_column_names();
        let sort_clauses = self.current_sort_clauses();
        let filter_clauses = self.current_filter_clauses();
        let (columns, rows) = self.db_ref()?.export_table_rows(
            &table_name,
            &visible_columns,
            &sort_clauses,
            &filter_clauses,
        )?;

        if columns.is_empty() {
            self.status_message = Some("Nothing to export".to_string());
            return Ok(());
        }

        let table_label = self.display_table_name(&table_name);
        let path = unique_export_path(&default_export_path(&table_label));
        std::fs::write(&path, serialize_csv(&columns, &rows))?;
        self.status_message = Some(format!(
            "Exported {} row(s) to {}",
            rows.len(),
            path.display()
        ));
        Ok(())
    }
}

fn unique_export_path(path: &Path) -> std::path::PathBuf {
    if !path.exists() {
        return path.to_path_buf();
    }

    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("export");
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("csv");

    for index in 1..=999 {
        let candidate = parent.join(format!("{stem}-{index}.{extension}"));
        if !candidate.exists() {
            return candidate;
        }
    }

    path.to_path_buf()
}

#[cfg(test)]
mod tests;
