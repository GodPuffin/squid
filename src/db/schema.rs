use anyhow::{Context, Result, anyhow};
use rusqlite::types::Value;
use rusqlite::{Connection, params_from_iter};

use super::{ColumnInfo, Database, ForeignKeyInfo, TableDetails};
use crate::db::query::quote_identifier;

impl Database {
    pub fn table_details(&self, table_name: &str) -> Result<TableDetails> {
        let safe_table_name = quote_identifier(table_name);
        let columns = self.column_info(table_name)?;
        let total_rows = count_rows(&self.conn, &safe_table_name, "", &[])?;
        let create_sql = self.conn.query_row(
            "SELECT sql
             FROM (
                 SELECT sql, 0 AS priority
                 FROM sqlite_temp_master
                 WHERE type = 'table' AND name = ?1
                 UNION ALL
                 SELECT sql, 1 AS priority
                 FROM sqlite_master
                 WHERE type = 'table' AND name = ?1
             )
             ORDER BY priority
             LIMIT 1",
            [table_name],
            |row| row.get::<_, Option<String>>(0),
        )?;

        Ok(TableDetails {
            create_sql,
            columns,
            total_rows,
        })
    }

    pub(crate) fn list_columns(&self, table_name: &str) -> Result<Vec<String>> {
        let pragma = format!("PRAGMA table_info({})", quote_identifier(table_name));
        let mut stmt = self.conn.prepare(&pragma)?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;

        let mut columns = Vec::new();
        for row in rows {
            columns.push(row?);
        }

        Ok(columns)
    }

    pub(crate) fn column_info(&self, table_name: &str) -> Result<Vec<ColumnInfo>> {
        let pragma = format!("PRAGMA table_info({})", quote_identifier(table_name));
        let mut stmt = self.conn.prepare(&pragma)?;
        let rows = stmt.query_map([], |row| {
            let not_null = row.get::<_, i64>(3)? != 0;
            let is_primary_key = row.get::<_, i64>(5)? != 0;
            Ok(ColumnInfo {
                name: row.get::<_, String>(1)?,
                data_type: row.get::<_, String>(2)?,
                not_null,
                default_value: row.get::<_, Option<String>>(4)?,
                is_primary_key,
            })
        })?;

        let mut columns = Vec::new();
        for row in rows {
            columns.push(row?);
        }

        Ok(columns)
    }

    pub(crate) fn foreign_key_info(&self, table_name: &str) -> Result<Vec<ForeignKeyInfo>> {
        let pragma = format!("PRAGMA foreign_key_list({})", quote_identifier(table_name));
        let mut stmt = self.conn.prepare(&pragma)?;
        let rows = stmt.query_map([], |row| {
            Ok(ForeignKeyInfo {
                target_table: row.get::<_, String>(2)?,
                from_column: row.get::<_, String>(3)?,
                target_column: row.get::<_, String>(4)?,
            })
        })?;

        let mut foreign_keys = Vec::new();
        for row in rows {
            let foreign_key = row?;
            if !foreign_key.from_column.is_empty() && !foreign_key.target_column.is_empty() {
                foreign_keys.push(foreign_key);
            }
        }

        Ok(foreign_keys)
    }
}

pub(crate) fn count_rows(
    conn: &Connection,
    safe_table_name: &str,
    where_clause: &str,
    filter_params: &[Value],
) -> Result<usize> {
    let sql = format!("SELECT COUNT(*) FROM {safe_table_name}{where_clause}");
    let count = conn
        .query_row(&sql, params_from_iter(filter_params.iter()), |row| {
            row.get::<_, i64>(0)
        })
        .with_context(|| format!("failed to count rows for table {safe_table_name}"))?;

    usize::try_from(count).map_err(|_| anyhow!("row count overflowed usize"))
}
