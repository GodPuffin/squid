use anyhow::Result;
use rusqlite::params;

use super::query::{build_filter_where, quote_identifier};
use super::value::format_value;
use super::{Database, FilterClause, SearchHit, TableSummary};

impl Database {
    pub fn search_table(
        &self,
        table_name: &str,
        visible_columns: &[String],
        filter_clauses: &[FilterClause],
        query: &str,
        limit: usize,
    ) -> Result<Vec<SearchHit>> {
        if query.trim().is_empty() {
            return Ok(Vec::new());
        }

        let safe_table_name = quote_identifier(table_name);
        let columns = if visible_columns.is_empty() {
            self.list_columns(table_name)?
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
        let (where_clause, mut filter_params) = build_filter_where(filter_clauses);
        let sql = format!(
            "SELECT rowid, {select_list}
             FROM {safe_table_name}
             {where_clause}
             LIMIT ?"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let column_count = stmt.column_count().saturating_sub(1);
        let scan_limit = i64::try_from(limit.saturating_mul(10)).unwrap_or(i64::MAX);
        filter_params.push(rusqlite::types::Value::Integer(scan_limit));

        let row_iter = stmt.query_map(rusqlite::params_from_iter(filter_params.iter()), |row| {
            let rowid = row.get::<_, i64>(0)?;
            let mut values = Vec::with_capacity(column_count);
            for idx in 0..column_count {
                values.push(format_value(row.get_ref(idx + 1)?));
            }
            Ok((rowid, values))
        })?;

        let mut results = Vec::new();
        for row in row_iter {
            let (rowid, values) = row?;
            let summary = render_search_summary(&columns, &values);
            let matched_columns = values
                .iter()
                .map(|value| fuzzy_score(value, query).is_some())
                .collect::<Vec<_>>();
            if let Some(score) = fuzzy_score(&summary, query) {
                results.push(SearchHit {
                    table_name: table_name.to_string(),
                    rowid: Some(rowid),
                    row_label: format!("rowid {rowid}"),
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
        if query.trim().is_empty() {
            return Ok(Vec::new());
        }

        let columns = self.list_columns(table_name)?;
        if columns.is_empty() {
            return Ok(Vec::new());
        }

        let safe_table_name = quote_identifier(table_name);
        let select_list = columns
            .iter()
            .map(|column| quote_identifier(column))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT rowid, {select_list}
             FROM {safe_table_name}
             LIMIT ?1"
        );
        let mut stmt = match self.conn.prepare(&sql) {
            Ok(stmt) => stmt,
            Err(_) => return Ok(Vec::new()),
        };
        let column_count = stmt.column_count().saturating_sub(1);
        let scan_limit = i64::try_from(limit.saturating_mul(20)).unwrap_or(i64::MAX);
        let query_lower = query.to_lowercase();

        let row_iter = stmt.query_map(params![scan_limit], |row| {
            let rowid = row.get::<_, i64>(0)?;
            let mut values = Vec::with_capacity(column_count);
            for idx in 0..column_count {
                values.push(format_value(row.get_ref(idx + 1)?));
            }
            Ok((rowid, values))
        })?;

        let mut results = Vec::new();
        for row in row_iter {
            let (rowid, values) = row?;
            let summary = values.join(" | ");
            if let Some(score) = exact_match_score(&summary, &query_lower) {
                results.push(SearchHit {
                    table_name: table_name.to_string(),
                    rowid: Some(rowid),
                    row_label: format!("rowid {rowid}"),
                    values: Vec::new(),
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
}

fn render_search_summary(columns: &[String], values: &[String]) -> String {
    columns
        .iter()
        .zip(values.iter())
        .map(|(column, value)| format!("{column}: {}", truncate_search_value(value)))
        .collect::<Vec<_>>()
        .join(" | ")
}

fn truncate_search_value(value: &str) -> String {
    const MAX: usize = 80;
    if value.chars().count() <= MAX {
        value.to_string()
    } else {
        let truncated: String = value.chars().take(MAX.saturating_sub(1)).collect();
        format!("{truncated}…")
    }
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
mod tests {
    use super::{exact_match_score, fuzzy_score};

    #[test]
    fn fuzzy_score_prefers_tighter_match() {
        let compact = fuzzy_score("alphabet", "alp").unwrap();
        let loose = fuzzy_score("a long phrase with letters", "alp").unwrap();
        assert!(compact > loose);
    }

    #[test]
    fn exact_match_prefers_full_match_over_prefix() {
        let full = exact_match_score("actor", "actor").unwrap();
        let prefix = exact_match_score("actor_name", "actor").unwrap();
        assert!(full > prefix);
    }
}
