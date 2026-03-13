mod execute;
mod query;
mod schema;
mod search;
mod value;

use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::Connection;

#[derive(Debug, Clone)]
pub struct TableSummary {
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct ColumnInfo {
    pub name: String,
    pub data_type: String,
    pub not_null: bool,
    pub default_value: Option<String>,
    pub is_primary_key: bool,
}

#[derive(Debug, Clone)]
pub struct TableDetails {
    pub create_sql: Option<String>,
    pub columns: Vec<ColumnInfo>,
    pub total_rows: usize,
}

#[derive(Debug, Clone)]
pub struct RowPreview {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub total_rows: usize,
}

#[derive(Debug, Clone)]
pub struct SortClause {
    pub column_name: String,
    pub descending: bool,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum FilterMode {
    Contains,
    Equals,
    StartsWith,
    GreaterThan,
    LessThan,
    IsTrue,
    IsFalse,
}

#[derive(Debug, Clone)]
pub struct FilterClause {
    pub column_name: String,
    pub mode: FilterMode,
    pub value: String,
}

#[derive(Debug, Clone)]
pub struct SearchHit {
    pub table_name: String,
    pub rowid: Option<i64>,
    pub row_label: String,
    pub values: Vec<String>,
    pub matched_columns: Vec<bool>,
    pub haystack: String,
    pub score: i64,
}

#[derive(Debug, Clone)]
pub struct ForeignKeyInfo {
    pub from_column: String,
    pub target_table: String,
    pub target_column: String,
}

#[derive(Debug, Clone)]
pub struct RowRecord {
    pub row_label: String,
    pub fields: Vec<(String, String)>,
    pub foreign_keys: Vec<ForeignKeyInfo>,
}

#[derive(Debug, Clone)]
pub enum SqlExecutionResult {
    Rows {
        columns: Vec<String>,
        rows: Vec<Vec<String>>,
    },
    Statement {
        affected_rows: usize,
        description: String,
    },
}

impl RowPreview {
    pub fn empty() -> Self {
        Self {
            columns: Vec::new(),
            rows: Vec::new(),
            total_rows: 0,
        }
    }
}

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open_with_flags(
            path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE | rusqlite::OpenFlags::SQLITE_OPEN_URI,
        )
        .with_context(|| format!("failed to open database {}", path.display()))?;

        Ok(Self { conn })
    }

    pub fn list_tables(&self) -> Result<Vec<TableSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT name
             FROM sqlite_master
             WHERE type = 'table' AND name NOT LIKE 'sqlite_%'
             ORDER BY name",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(TableSummary {
                name: row.get::<_, String>(0)?,
            })
        })?;

        let mut tables = Vec::new();
        for row in rows {
            tables.push(row?);
        }

        Ok(tables)
    }
}
