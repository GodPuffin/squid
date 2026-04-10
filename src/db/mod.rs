mod execute;
mod query;
mod schema;
mod search;
mod value;

use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::{Connection, MAIN_DB, OpenFlags};

use crate::db::query::split_qualified_table_name;
use crate::db::schema::schema_catalog_table;

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
    pub row_offset: usize,
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
pub struct RowField {
    pub column_name: String,
    pub value: String,
    pub is_blob: bool,
}

#[derive(Debug, Clone)]
pub struct RowRecord {
    pub rowid: Option<i64>,
    pub row_label: String,
    pub fields: Vec<RowField>,
    pub foreign_keys: Vec<ForeignKeyInfo>,
}

#[derive(Debug, Clone)]
pub enum SqlExecutionResult {
    Rows {
        columns: Vec<String>,
        rows: Vec<Vec<String>>,
        is_mutation: bool,
        is_truncated: bool,
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
        let read_write_flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_URI;
        let conn = match Connection::open_with_flags(path, read_write_flags) {
            Ok(conn) => conn,
            Err(read_write_err) => {
                let read_only_flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI;
                Connection::open_with_flags(path, read_only_flags).with_context(|| {
                    format!(
                        "failed to open database {} for read-write or read-only access (read-write attempt failed: {read_write_err})",
                        path.display()
                    )
                })?
            }
        };
        Ok(Self { conn })
    }

    pub fn table_is_writable(&self, table_name: &str) -> Result<bool> {
        let schema = split_qualified_table_name(table_name)
            .map(|(schema, _)| schema)
            .unwrap_or_else(|| MAIN_DB.to_str().unwrap_or("main"));
        Ok(!self.conn.is_readonly(schema)?)
    }

    pub fn list_tables(&self) -> Result<Vec<TableSummary>> {
        let mut tables = Vec::new();
        for schema_name in self.list_attached_schemas()? {
            let schema_tables = self.list_tables_in_schema(&schema_name)?;
            tables.extend(schema_tables.into_iter().map(|table_name| TableSummary {
                name: format!("{schema_name}.{table_name}"),
            }));
        }
        tables.sort_by(|left, right| left.name.cmp(&right.name));

        Ok(tables)
    }

    fn list_attached_schemas(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare("PRAGMA database_list")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;

        let mut schemas = Vec::new();
        for row in rows {
            schemas.push(row?);
        }

        Ok(schemas)
    }

    fn list_tables_in_schema(&self, schema_name: &str) -> Result<Vec<String>> {
        let master_table = schema_catalog_table(schema_name);
        let sql = format!(
            "SELECT name
             FROM {master_table}
             WHERE type = 'table' AND name NOT LIKE 'sqlite_%'
             ORDER BY name"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;

        let mut tables = Vec::new();
        for row in rows {
            tables.push(row?);
        }

        Ok(tables)
    }
}

#[cfg(test)]
mod tests;
