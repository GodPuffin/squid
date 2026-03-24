use anyhow::{Context, Result, anyhow};
use rusqlite::types::Value;
use rusqlite::{Connection, params_from_iter};

use super::{ColumnInfo, Database, ForeignKeyInfo, TableDetails};
use crate::db::query::{quote_identifier, quote_table_name, split_qualified_table_name};

impl Database {
    pub fn table_details(&self, table_name: &str) -> Result<TableDetails> {
        let safe_table_name = quote_table_name(table_name);
        let columns = self.column_info(table_name)?;
        let total_rows = count_rows(&self.conn, &safe_table_name, "", &[])?;
        let create_sql = if let Some((schema, bare_name)) = split_qualified_table_name(table_name) {
            let master_table = schema_catalog_table(schema);
            let sql = format!(
                "SELECT sql
                 FROM {master_table}
                 WHERE type = 'table' AND name = ?1
                 LIMIT 1"
            );
            self.conn
                .query_row(&sql, [bare_name], |row| row.get::<_, Option<String>>(0))?
        } else {
            self.conn.query_row(
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
            )?
        };

        Ok(TableDetails {
            create_sql,
            columns,
            total_rows,
        })
    }

    pub(crate) fn list_columns(&self, table_name: &str) -> Result<Vec<String>> {
        let pragma = table_pragma_sql(table_name, "table_info");
        let mut stmt = self.conn.prepare(&pragma)?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;

        let mut columns = Vec::new();
        for row in rows {
            columns.push(row?);
        }

        Ok(columns)
    }

    pub(crate) fn column_info(&self, table_name: &str) -> Result<Vec<ColumnInfo>> {
        let pragma = table_pragma_sql(table_name, "table_info");
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
        let pragma = table_pragma_sql(table_name, "foreign_key_list");
        let source_schema = split_qualified_table_name(table_name).map(|(schema, _)| schema);
        let mut stmt = self.conn.prepare(&pragma)?;
        let rows = stmt.query_map([], |row| {
            let target_table = row.get::<_, String>(2)?;
            Ok(ForeignKeyInfo {
                target_table: qualify_foreign_target_table(source_schema, &target_table),
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

fn qualify_foreign_target_table(source_schema: Option<&str>, target_table: &str) -> String {
    if split_qualified_table_name(target_table).is_some() {
        target_table.to_string()
    } else if let Some(schema) = source_schema {
        format!("{schema}.{target_table}")
    } else {
        target_table.to_string()
    }
}

pub(crate) fn schema_catalog_table(schema_name: &str) -> String {
    if schema_name == "temp" {
        "sqlite_temp_master".to_string()
    } else {
        format!("{}.sqlite_master", quote_identifier(schema_name))
    }
}

fn table_pragma_sql(table_name: &str, pragma_name: &str) -> String {
    if let Some((schema, bare_name)) = split_qualified_table_name(table_name) {
        format!(
            "PRAGMA {}.{}({})",
            quote_identifier(schema),
            pragma_name,
            quote_identifier(bare_name)
        )
    } else {
        format!("PRAGMA {pragma_name}({})", quote_identifier(table_name))
    }
}

#[cfg(test)]
mod tests;

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
