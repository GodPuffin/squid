use anyhow::Result;
use rusqlite::{params_from_iter, types::Value};

use super::query::{build_filter_where, build_order_by, quote_identifier, quote_table_name};
use super::value::format_value;
use super::{Database, FilterClause, SearchHit, SortClause, TableSummary};

// Keep a small overage so exhaustive scans do not retain every match in memory.
const SEARCH_RESULT_BUFFER: usize = 64;
const CURRENT_TABLE_EXACT_MATCH_BOOST: i64 = 1_000_000;

struct SearchScan<'a> {
    safe_table_name: &'a str,
    select_list: &'a str,
    where_clause: &'a str,
    order_by: &'a str,
    rowid_alias: Option<&'a str>,
}

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
        let mut results = Vec::with_capacity(limit.min(SEARCH_RESULT_BUFFER));
        self.scan_search_rows(
            SearchScan {
                safe_table_name: &safe_table_name,
                select_list: &select_list,
                where_clause: &where_clause,
                order_by: &order_by,
                rowid_alias,
            },
            &filter_params,
            |index, rowid, values| {
                let summary = render_search_summary(&columns, &values);
                let column_scores = values
                    .iter()
                    .map(|value| current_table_match_score(value, query))
                    .collect::<Vec<_>>();
                let matched_columns = column_scores
                    .iter()
                    .map(|score| score.is_some())
                    .collect::<Vec<_>>();
                if let Some(score) = column_scores.iter().flatten().copied().max() {
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
                    trim_current_table_hits(&mut results, limit);
                }
                Ok(())
            },
        )?;

        sort_current_table_hits(&mut results);
        results.truncate(limit);
        Ok(results)
    }

    pub fn search_tables(
        &self,
        tables: &[TableSummary],
        query: &str,
        limit: usize,
    ) -> Result<Vec<SearchHit>> {
        if query.trim().is_empty() || limit == 0 {
            return Ok(Vec::new());
        }

        let mut all_results = Vec::new();

        for table in tables {
            let mut hits = self.search_table_exact(&table.name, query, limit)?;
            all_results.append(&mut hits);
        }

        sort_all_table_hits(&mut all_results);
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
        let mut results = Vec::with_capacity(limit.min(SEARCH_RESULT_BUFFER));
        self.scan_search_rows(
            SearchScan {
                safe_table_name: &safe_table_name,
                select_list: &select_list,
                where_clause: "",
                order_by: "",
                rowid_alias,
            },
            &[],
            |index, rowid, values| {
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
                    trim_exact_table_hits(&mut results, limit);
                }
                Ok(())
            },
        )?;

        sort_exact_table_hits(&mut results);
        results.truncate(limit);
        Ok(results)
    }

    fn scan_search_rows<F>(
        &self,
        scan: SearchScan<'_>,
        filter_params: &[Value],
        mut on_row: F,
    ) -> Result<()>
    where
        F: FnMut(usize, Option<i64>, Vec<String>) -> Result<()>,
    {
        if let Some(rowid_alias) = scan.rowid_alias {
            let sql = format!(
                "SELECT {rowid_alias}, {select_list}
                 FROM {safe_table_name}
                 {where_clause}
                 {order_by}",
                select_list = scan.select_list,
                safe_table_name = scan.safe_table_name,
                where_clause = scan.where_clause,
                order_by = scan.order_by
            );
            if let Ok(mut stmt) = self.conn.prepare(&sql) {
                let column_count = stmt.column_count().saturating_sub(1);
                let mut rows = stmt.query(params_from_iter(filter_params.iter()))?;
                let mut index = 0usize;
                while let Some(row) = rows.next()? {
                    let rowid = row.get::<_, i64>(0)?;
                    let mut values = Vec::with_capacity(column_count);
                    for idx in 0..column_count {
                        values.push(format_value(row.get_ref(idx + 1)?));
                    }
                    on_row(index, Some(rowid), values)?;
                    index += 1;
                }
                return Ok(());
            }
        }

        let sql = format!(
            "SELECT {select_list}
             FROM {safe_table_name}
             {where_clause}
             {order_by}",
            select_list = scan.select_list,
            safe_table_name = scan.safe_table_name,
            where_clause = scan.where_clause,
            order_by = scan.order_by
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let column_count = stmt.column_count();
        let mut rows = stmt.query(params_from_iter(filter_params.iter()))?;
        let mut index = 0usize;
        while let Some(row) = rows.next()? {
            let mut values = Vec::with_capacity(column_count);
            for idx in 0..column_count {
                values.push(format_value(row.get_ref(idx)?));
            }
            on_row(index, None, values)?;
            index += 1;
        }
        Ok(())
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

fn current_table_match_score(value: &str, query: &str) -> Option<i64> {
    exact_match_score(value, query)
        .map(|score| score.saturating_add(CURRENT_TABLE_EXACT_MATCH_BOOST))
        .or_else(|| fuzzy_score(value, query))
}

fn trim_current_table_hits(results: &mut Vec<SearchHit>, limit: usize) {
    trim_search_hits(results, limit, sort_current_table_hits);
}

fn trim_exact_table_hits(results: &mut Vec<SearchHit>, limit: usize) {
    trim_search_hits(results, limit, sort_exact_table_hits);
}

fn trim_search_hits(results: &mut Vec<SearchHit>, limit: usize, sorter: fn(&mut [SearchHit])) {
    let retain_limit = limit.saturating_add(SEARCH_RESULT_BUFFER);
    if results.len() > retain_limit {
        sorter(results);
        results.truncate(limit);
    }
}

fn sort_current_table_hits(results: &mut [SearchHit]) {
    results.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.row_label.cmp(&right.row_label))
    });
}

fn sort_exact_table_hits(results: &mut [SearchHit]) {
    results.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.row_label.cmp(&right.row_label))
    });
}

fn sort_all_table_hits(results: &mut [SearchHit]) {
    results.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.table_name.cmp(&right.table_name))
            .then_with(|| left.row_label.cmp(&right.row_label))
    });
}

pub(crate) fn fuzzy_score(haystack: &str, query: &str) -> Option<i64> {
    score_fuzzy_match(&fuzzy_match_positions(haystack, query))
}

pub(crate) fn fuzzy_match_positions(haystack: &str, query: &str) -> Vec<usize> {
    let haystack_lower: Vec<char> = haystack.to_lowercase().chars().collect();
    let query_lower: Vec<char> = query.to_lowercase().chars().collect();

    if haystack_lower.is_empty() || query_lower.is_empty() {
        return Vec::new();
    }

    let mut best: Option<(i64, Vec<usize>)> = None;
    for (start_idx, candidate) in haystack_lower.iter().enumerate() {
        if *candidate != query_lower[0] {
            continue;
        }

        let mut positions = vec![start_idx];
        let mut search_index = start_idx + 1;
        let mut matched = true;

        for needle in query_lower.iter().skip(1) {
            let Some(idx) = haystack_lower
                .iter()
                .enumerate()
                .skip(search_index)
                .find_map(|(idx, candidate)| (*candidate == *needle).then_some(idx))
            else {
                matched = false;
                break;
            };
            positions.push(idx);
            search_index = idx + 1;
        }

        if !matched {
            continue;
        }

        let score = score_fuzzy_match(&positions).unwrap_or(i64::MIN);
        let replace = best.as_ref().is_none_or(|(best_score, best_positions)| {
            score > *best_score || (score == *best_score && positions < *best_positions)
        });
        if replace {
            best = Some((score, positions));
        }
    }

    best.map(|(_, positions)| positions).unwrap_or_default()
}

fn score_fuzzy_match(positions: &[usize]) -> Option<i64> {
    let first = *positions.first()?;
    let last = *positions.last()?;
    let query_len = positions.len();
    let span = last.saturating_sub(first) + 1;
    let gaps = span.saturating_sub(query_len);
    let consecutive = positions
        .windows(2)
        .filter(|window| window[1] == window[0] + 1)
        .count();

    let mut score = i64::try_from(query_len).unwrap_or(0) * 100;
    score += i64::try_from(consecutive).unwrap_or(0) * 40;
    score -= i64::try_from(gaps).unwrap_or(0) * 25;
    score -= i64::try_from(first).unwrap_or(0) * 2;
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
