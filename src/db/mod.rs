mod execute;
mod query;
mod schema;
mod search;
mod value;

use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::{Connection, OpenFlags};

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
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use rusqlite::Connection;

    use super::Database;

    #[test]
    fn list_tables_includes_attached_schemas() {
        let main_path = temp_db_path("main");
        let attached_path = temp_db_path("attached");

        let conn = Connection::open(&main_path).expect("create main db");
        conn.execute("CREATE TABLE main_only(id INTEGER PRIMARY KEY)", [])
            .expect("create main table");
        conn.execute(
            "ATTACH DATABASE ?1 AS other",
            [attached_path.to_string_lossy().into_owned()],
        )
        .expect("attach db");
        conn.execute("CREATE TABLE other.other_only(id INTEGER PRIMARY KEY)", [])
            .expect("create attached table");
        drop(conn);

        let db = Database::open(&main_path).expect("open db");
        db.conn
            .execute(
                "ATTACH DATABASE ?1 AS other",
                [attached_path.to_string_lossy().into_owned()],
            )
            .expect("attach db on app connection");
        let tables = db.list_tables().expect("list tables");
        let names = tables
            .into_iter()
            .map(|table| table.name)
            .collect::<Vec<_>>();

        assert!(names.contains(&"main.main_only".to_string()));
        assert!(names.contains(&"other.other_only".to_string()));

        let _ = fs::remove_file(main_path);
        let _ = fs::remove_file(attached_path);
    }

    fn temp_db_path(label: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("squid-db-{label}-{stamp}.sqlite"))
    }
}
