use anyhow::Result;
use rusqlite::{params_from_iter, types::Value};

use super::query::{build_filter_where, build_order_by, quote_identifier, quote_table_name};
use super::value::format_value;
use super::{Database, FilterClause, SearchHit, SortClause, TableSummary};

const CURRENT_TABLE_SCAN_MULTIPLIER: usize = 100;
const CURRENT_TABLE_SCAN_MIN_ROWS: usize = 1_000;
const CURRENT_TABLE_SCAN_MAX_ROWS: usize = 25_000;

const ALL_TABLE_SCAN_MULTIPLIER: usize = 50;
const ALL_TABLE_SCAN_MIN_ROWS: usize = 500;
const ALL_TABLE_SCAN_MAX_ROWS: usize = 10_000;

impl Database {
    pub fn search_table(
        &self,
        table_name: &str,
        visible_columns: &[String],
        sort_clauses: &[SortClause],
        filter_clauses: &[FilterClause],
        query: &str,
        limit: usize,
    ) -> Result<Vec<SearchHit>> {
        if query.trim().is_empty() || limit == 0 {
            return Ok(Vec::new());
        }

        let safe_table_name = quote_table_name(table_name);
        let table_columns = self.list_columns(table_name)?;
        let rowid_alias = self.rowid_alias(table_name)?;
        let columns = if visible_columns.is_empty() {
            table_columns.clone()
        } else {
            visible_columns.to_vec()
        };

        if columns.is_empty() {
            return Ok(Vec::new());
        }

        let select_list = columns
            .iter()
            .map(|column| quote_identifier(column))
            .collect::<Vec<_>>()
            .join(", ");
        let (where_clause, filter_params) = build_filter_where(filter_clauses);
        let order_by = build_order_by(sort_clauses);
        let scan_limit = bounded_scan_limit(
            limit,
            CURRENT_TABLE_SCAN_MULTIPLIER,
            CURRENT_TABLE_SCAN_MIN_ROWS,
            CURRENT_TABLE_SCAN_MAX_ROWS,
        );
        let row_iter = self.scan_search_rows(
            &safe_table_name,
            &select_list,
            &where_clause,
            &order_by,
            &filter_params,
            Some(scan_limit),
            rowid_alias,
        )?;

        let mut results = Vec::new();
        for (index, row) in row_iter.into_iter().enumerate() {
            let (rowid, values) = row;
            let summary = render_search_summary(&columns, &values);
            let matched_columns = values
                .iter()
                .map(|value| fuzzy_score(value, query).is_some())
                .collect::<Vec<_>>();
            if let Some(score) = fuzzy_score(&summary, query) {
                let row_label = rowid
                    .map(|rowid| format!("rowid {rowid}"))
                    .unwrap_or_else(|| format!("row {}", index + 1));
                results.push(SearchHit {
                    table_name: table_name.to_string(),
                    rowid,
                    row_offset: index,
                    row_label,
                    values,
                    matched_columns,
                    haystack: summary,
                    score,
                });
            }
        }

        results.sort_by(|left, right| {
            right
                .score
                .cmp(&left.score)
                .then_with(|| left.row_label.cmp(&right.row_label))
        });
        results.truncate(limit);
        Ok(results)
    }

    pub fn search_tables(
        &self,
        tables: &[TableSummary],
        query: &str,
        limit: usize,
    ) -> Result<Vec<SearchHit>> {
        let mut all_results = Vec::new();

        for table in tables {
            let mut hits = self.search_table_exact(&table.name, query, limit.min(80))?;
            all_results.append(&mut hits);
        }

        all_results.sort_by(|left, right| {
            right
                .score
                .cmp(&left.score)
                .then_with(|| left.table_name.cmp(&right.table_name))
                .then_with(|| left.row_label.cmp(&right.row_label))
        });
        all_results.truncate(limit);
        Ok(all_results)
    }

    fn search_table_exact(
        &self,
        table_name: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<SearchHit>> {
        if query.trim().is_empty() || limit == 0 {
            return Ok(Vec::new());
        }

        let columns = self.list_columns(table_name)?;
        if columns.is_empty() {
            return Ok(Vec::new());
        }
        let rowid_alias = self.rowid_alias(table_name)?;

        let safe_table_name = quote_table_name(table_name);
        let select_list = columns
            .iter()
            .map(|column| quote_identifier(column))
            .collect::<Vec<_>>()
            .join(", ");
        let query_lower = query.to_lowercase();
        let scan_limit = bounded_scan_limit(
            limit,
            ALL_TABLE_SCAN_MULTIPLIER,
            ALL_TABLE_SCAN_MIN_ROWS,
            ALL_TABLE_SCAN_MAX_ROWS,
        );
        let row_iter = self.scan_search_rows(
            &safe_table_name,
            &select_list,
            "",
            "",
            &[],
            Some(scan_limit),
            rowid_alias,
        )?;

        let mut results = Vec::new();
        for (index, row) in row_iter.into_iter().enumerate() {
            let (rowid, values) = row;
            let summary = values.join(" | ");
            if let Some(score) = exact_match_score(&summary, &query_lower) {
                let row_label = rowid
                    .map(|rowid| format!("rowid {rowid}"))
                    .unwrap_or_else(|| format!("row {}", index + 1));
                results.push(SearchHit {
                    table_name: table_name.to_string(),
                    rowid,
                    row_offset: index,
                    row_label,
                    values,
                    matched_columns: Vec::new(),
                    haystack: summary,
                    score,
                });
            }
        }

        results.sort_by(|left, right| {
            right
                .score
                .cmp(&left.score)
                .then_with(|| left.row_label.cmp(&right.row_label))
        });
        results.truncate(limit);
        Ok(results)
    }

    fn scan_search_rows(
        &self,
        safe_table_name: &str,
        select_list: &str,
        where_clause: &str,
        order_by: &str,
        filter_params: &[Value],
        scan_limit: Option<i64>,
        rowid_alias: Option<&str>,
    ) -> Result<Vec<(Option<i64>, Vec<String>)>> {
        let limit_clause = scan_limit
            .map(|_| "LIMIT ?")
            .unwrap_or_default()
            .to_string();

        if let Some(rowid_alias) = rowid_alias {
            let sql = format!(
                "SELECT {rowid_alias}, {select_list}
                 FROM {safe_table_name}
                 {where_clause}
                 {order_by}
                 {limit_clause}"
            );
            if let Ok(mut stmt) = self.conn.prepare(&sql) {
                let column_count = stmt.column_count().saturating_sub(1);
                let mut params = filter_params.to_vec();
                if let Some(scan_limit) = scan_limit {
                    params.push(Value::Integer(scan_limit));
                }
                let rows = stmt.query_map(params_from_iter(params.iter()), |row| {
                    let rowid = row.get::<_, i64>(0)?;
                    let mut values = Vec::with_capacity(column_count);
                    for idx in 0..column_count {
                        values.push(format_value(row.get_ref(idx + 1)?));
                    }
                    Ok((Some(rowid), values))
                })?;
                return Ok(rows.collect::<Result<Vec<_>, _>>()?);
            }
        }

        let sql = format!(
            "SELECT {select_list}
             FROM {safe_table_name}
             {where_clause}
             {order_by}
             {limit_clause}"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let column_count = stmt.column_count();
        let mut params = filter_params.to_vec();
        if let Some(scan_limit) = scan_limit {
            params.push(Value::Integer(scan_limit));
        }
        let rows = stmt.query_map(params_from_iter(params.iter()), |row| {
            let mut values = Vec::with_capacity(column_count);
            for idx in 0..column_count {
                values.push(format_value(row.get_ref(idx)?));
            }
            Ok((None, values))
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }
}

fn render_search_summary(columns: &[String], values: &[String]) -> String {
    columns
        .iter()
        .zip(values.iter())
        .map(|(column, value)| format!("{column}: {value}"))
        .collect::<Vec<_>>()
        .join(" | ")
}

fn bounded_scan_limit(limit: usize, multiplier: usize, min_rows: usize, max_rows: usize) -> i64 {
    let expanded = limit.saturating_mul(multiplier);
    let bounded = expanded.max(min_rows).min(max_rows);
    i64::try_from(bounded).unwrap_or(i64::MAX)
}

pub(crate) fn fuzzy_score(haystack: &str, query: &str) -> Option<i64> {
    if query.is_empty() {
        return None;
    }

    let haystack_chars: Vec<char> = haystack.chars().collect();
    let haystack_lower: Vec<char> = haystack.to_lowercase().chars().collect();
    let query_lower: Vec<char> = query.to_lowercase().chars().collect();

    if haystack_lower.is_empty() || query_lower.is_empty() {
        return None;
    }

    let mut last_match = None;
    let mut score = 0_i64;
    let mut search_index = 0_usize;

    for needle in query_lower {
        let mut found = None;
        for (idx, candidate) in haystack_lower.iter().enumerate().skip(search_index) {
            if *candidate == needle {
                found = Some(idx);
                break;
            }
        }

        let idx = found?;
        score += 10;
        if let Some(previous) = last_match {
            if idx == previous + 1 {
                score += 8;
            } else {
                score -= i64::try_from(idx - previous).unwrap_or(0);
            }
        } else {
            score += i64::try_from(haystack_chars.len().saturating_sub(idx)).unwrap_or(0);
        }

        last_match = Some(idx);
        search_index = idx + 1;
    }

    Some(score)
}

pub(crate) fn exact_match_score(haystack: &str, query: &str) -> Option<i64> {
    let haystack_lower = haystack.to_lowercase();
    let query_lower = query.to_lowercase();

    let position = haystack_lower.find(&query_lower)?;
    let mut score = 1_000_i64;

    if haystack_lower == query_lower {
        score += 5_000;
    } else if haystack_lower.starts_with(&query_lower) {
        score += 2_000;
    }

    score -= i64::try_from(position).unwrap_or(0);
    score -= i64::try_from(haystack_lower.len().saturating_sub(query_lower.len())).unwrap_or(0);
    Some(score)
}

#[cfg(test)]
mod tests;
