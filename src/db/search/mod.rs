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

#[derive(Debug, Clone)]
struct SearchScanPlan {
    table_name: String,
    columns: Vec<String>,
    safe_table_name: String,
    select_list: String,
    where_clause: String,
    order_by: String,
    rowid_alias: Option<String>,
    filter_params: Vec<Value>,
}

#[derive(Debug, Clone)]
struct ExactTableSearchPlan {
    table_name: String,
    scan: SearchScanPlan,
}

#[derive(Debug)]
struct SearchRowBatch {
    rows: Vec<SearchRowBatchRow>,
    exhausted: bool,
}

#[derive(Debug)]
struct SearchRowBatchRow {
    rowid: Option<i64>,
    values: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
enum DeferredSearchCursor {
    Offset(usize),
    Rowid(Option<i64>),
}

#[derive(Debug)]
pub(crate) struct DeferredCurrentTableSearch {
    plan: SearchScanPlan,
    query: String,
    cursor: DeferredSearchCursor,
    next_row_offset: usize,
    results: Vec<SearchHit>,
    limit: usize,
}

#[derive(Debug)]
pub(crate) struct DeferredAllTablesSearch {
    tables: Vec<ExactTableSearchPlan>,
    query_lower: String,
    table_index: usize,
    cursor: DeferredSearchCursor,
    next_row_offset: usize,
    results: Vec<SearchHit>,
    limit: usize,
}

#[derive(Debug)]
pub(crate) enum DeferredSearchWork {
    CurrentTable(DeferredCurrentTableSearch),
    AllTables(DeferredAllTablesSearch),
}

impl DeferredSearchWork {
    pub(crate) fn step(&mut self, db: &Database, row_limit: usize) -> Result<bool> {
        match self {
            Self::CurrentTable(search) => search.step(db, row_limit),
            Self::AllTables(search) => search.step(db, row_limit),
        }
    }

    pub(crate) fn into_results(self) -> Vec<SearchHit> {
        match self {
            Self::CurrentTable(search) => search.results,
            Self::AllTables(search) => search.results,
        }
    }
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

        let Some(plan) = self.build_current_table_search_plan(
            table_name,
            visible_columns,
            sort_clauses,
            filter_clauses,
        )?
        else {
            return Ok(Vec::new());
        };

        let columns = plan.columns.clone();
        let mut results = Vec::with_capacity(limit.min(SEARCH_RESULT_BUFFER));
        self.scan_search_rows(
            SearchScan {
                safe_table_name: &plan.safe_table_name,
                select_list: &plan.select_list,
                where_clause: &plan.where_clause,
                order_by: &plan.order_by,
                rowid_alias: plan.rowid_alias.as_deref(),
            },
            &plan.filter_params,
            |index, rowid, values| {
                let summary = render_search_summary(&columns, &values);
                let preview = render_search_preview(&columns, &values);
                let summary_score = fuzzy_score(&summary, query);
                let column_scores = values
                    .iter()
                    .map(|value| current_table_match_score(value, query))
                    .collect::<Vec<_>>();
                let matched_columns = column_scores
                    .iter()
                    .map(|score| score.is_some())
                    .collect::<Vec<_>>();
                let score = column_scores
                    .iter()
                    .flatten()
                    .copied()
                    .max()
                    .into_iter()
                    .chain(summary_score)
                    .max();
                if let Some(score) = score {
                    let row_label = rowid
                        .map(|rowid| format!("rowid {rowid}"))
                        .unwrap_or_else(|| format!("row {}", index + 1));
                    results.push(SearchHit {
                        table_name: plan.table_name.clone(),
                        rowid,
                        row_offset: index,
                        row_label,
                        values,
                        matched_columns,
                        haystack: preview,
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
            trim_all_table_hits(&mut all_results, limit);
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

        let Some(plan) = self.build_exact_table_search_plan(table_name)? else {
            return Ok(Vec::new());
        };
        let query_lower = query.to_lowercase();
        let mut results = Vec::with_capacity(limit.min(SEARCH_RESULT_BUFFER));
        self.scan_search_rows(
            SearchScan {
                safe_table_name: &plan.safe_table_name,
                select_list: &plan.select_list,
                where_clause: &plan.where_clause,
                order_by: &plan.order_by,
                rowid_alias: plan.rowid_alias.as_deref(),
            },
            &[],
            |index, rowid, values| {
                let summary = values.join(" | ");
                if let Some(score) = exact_match_score(&summary, &query_lower) {
                    let row_label = rowid
                        .map(|rowid| format!("rowid {rowid}"))
                        .unwrap_or_else(|| format!("row {}", index + 1));
                    results.push(SearchHit {
                        table_name: plan.table_name.clone(),
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

    pub(crate) fn start_deferred_table_search(
        &self,
        table_name: &str,
        visible_columns: &[String],
        sort_clauses: &[SortClause],
        filter_clauses: &[FilterClause],
        query: &str,
        limit: usize,
    ) -> Result<Option<DeferredSearchWork>> {
        if query.trim().is_empty() || limit == 0 {
            return Ok(None);
        }

        let Some(plan) = self.build_current_table_search_plan(
            table_name,
            visible_columns,
            sort_clauses,
            filter_clauses,
        )?
        else {
            return Ok(None);
        };

        Ok(Some(DeferredSearchWork::CurrentTable(
            DeferredCurrentTableSearch {
                cursor: deferred_current_table_cursor(&plan),
                plan,
                query: query.to_string(),
                next_row_offset: 0,
                results: Vec::with_capacity(limit.min(SEARCH_RESULT_BUFFER)),
                limit,
            },
        )))
    }

    pub(crate) fn start_deferred_all_tables_search(
        &self,
        tables: &[TableSummary],
        query: &str,
        limit: usize,
    ) -> Result<Option<DeferredSearchWork>> {
        if query.trim().is_empty() || limit == 0 {
            return Ok(None);
        }

        let mut table_plans = Vec::new();
        for table in tables {
            if let Some(plan) = self.build_exact_table_search_plan(&table.name)? {
                table_plans.push(ExactTableSearchPlan {
                    table_name: table.name.clone(),
                    scan: plan,
                });
            }
        }

        Ok(Some(DeferredSearchWork::AllTables(
            DeferredAllTablesSearch {
                cursor: table_plans
                    .first()
                    .map(deferred_exact_table_cursor)
                    .unwrap_or(DeferredSearchCursor::Offset(0)),
                tables: table_plans,
                query_lower: query.to_lowercase(),
                table_index: 0,
                next_row_offset: 0,
                results: Vec::with_capacity(limit.min(SEARCH_RESULT_BUFFER)),
                limit,
            },
        )))
    }

    fn build_current_table_search_plan(
        &self,
        table_name: &str,
        visible_columns: &[String],
        sort_clauses: &[SortClause],
        filter_clauses: &[FilterClause],
    ) -> Result<Option<SearchScanPlan>> {
        let table_columns = self.list_columns(table_name)?;
        let columns = if visible_columns.is_empty() {
            table_columns
        } else {
            visible_columns.to_vec()
        };
        if columns.is_empty() {
            return Ok(None);
        }

        let (where_clause, filter_params) = build_filter_where(filter_clauses);
        Ok(Some(SearchScanPlan {
            table_name: table_name.to_string(),
            columns: columns.clone(),
            safe_table_name: quote_table_name(table_name),
            select_list: columns
                .iter()
                .map(|column| quote_identifier(column))
                .collect::<Vec<_>>()
                .join(", "),
            where_clause,
            order_by: build_order_by(sort_clauses),
            rowid_alias: self.selectable_rowid_alias(table_name)?.map(str::to_owned),
            filter_params,
        }))
    }

    fn build_exact_table_search_plan(&self, table_name: &str) -> Result<Option<SearchScanPlan>> {
        let columns = self.list_columns(table_name)?;
        if columns.is_empty() {
            return Ok(None);
        }

        Ok(Some(SearchScanPlan {
            table_name: table_name.to_string(),
            columns: columns.clone(),
            safe_table_name: quote_table_name(table_name),
            select_list: columns
                .iter()
                .map(|column| quote_identifier(column))
                .collect::<Vec<_>>()
                .join(", "),
            where_clause: String::new(),
            order_by: String::new(),
            rowid_alias: self.selectable_rowid_alias(table_name)?.map(str::to_owned),
            filter_params: Vec::new(),
        }))
    }

    fn selectable_rowid_alias(&self, table_name: &str) -> Result<Option<&'static str>> {
        let Some(rowid_alias) = self.rowid_alias(table_name)? else {
            return Ok(None);
        };

        let sql = format!(
            "SELECT {rowid_alias}
             FROM {safe_table_name}
             LIMIT 1",
            safe_table_name = quote_table_name(table_name)
        );

        if self.conn.prepare(&sql).is_ok() {
            Ok(Some(rowid_alias))
        } else {
            Ok(None)
        }
    }

    fn scan_search_rows_chunk(
        &self,
        plan: &SearchScanPlan,
        start_offset: usize,
        row_limit: usize,
    ) -> Result<SearchRowBatch> {
        if row_limit == 0 {
            return Ok(SearchRowBatch {
                rows: Vec::new(),
                exhausted: true,
            });
        }

        let fetch_limit = row_limit.saturating_add(1);
        if let Some(rowid_alias) = plan.rowid_alias.as_deref() {
            let sql = format!(
                "SELECT {rowid_alias}, {select_list}
                 FROM {safe_table_name}
                 {where_clause}
                 {order_by}
                 LIMIT {fetch_limit} OFFSET {start_offset}",
                select_list = plan.select_list,
                safe_table_name = plan.safe_table_name,
                where_clause = plan.where_clause,
                order_by = plan.order_by,
            );
            if let Ok(mut stmt) = self.conn.prepare(&sql) {
                let column_count = stmt.column_count().saturating_sub(1);
                let mut rows = stmt.query(params_from_iter(plan.filter_params.iter()))?;
                let mut batch_rows = Vec::with_capacity(fetch_limit.min(row_limit));
                while let Some(row) = rows.next()? {
                    let rowid = row.get::<_, i64>(0)?;
                    let mut values = Vec::with_capacity(column_count);
                    for idx in 0..column_count {
                        values.push(format_value(row.get_ref(idx + 1)?));
                    }
                    batch_rows.push(SearchRowBatchRow {
                        rowid: Some(rowid),
                        values,
                    });
                }
                let exhausted = batch_rows.len() <= row_limit;
                if !exhausted {
                    batch_rows.truncate(row_limit);
                }
                return Ok(SearchRowBatch {
                    rows: batch_rows,
                    exhausted,
                });
            }
        }

        let sql = format!(
            "SELECT {select_list}
             FROM {safe_table_name}
             {where_clause}
             {order_by}
             LIMIT {fetch_limit} OFFSET {start_offset}",
            select_list = plan.select_list,
            safe_table_name = plan.safe_table_name,
            where_clause = plan.where_clause,
            order_by = plan.order_by,
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let column_count = stmt.column_count();
        let mut rows = stmt.query(params_from_iter(plan.filter_params.iter()))?;
        let mut batch_rows = Vec::with_capacity(fetch_limit.min(row_limit));
        while let Some(row) = rows.next()? {
            let mut values = Vec::with_capacity(column_count);
            for idx in 0..column_count {
                values.push(format_value(row.get_ref(idx)?));
            }
            batch_rows.push(SearchRowBatchRow {
                rowid: None,
                values,
            });
        }
        let exhausted = batch_rows.len() <= row_limit;
        if !exhausted {
            batch_rows.truncate(row_limit);
        }
        Ok(SearchRowBatch {
            rows: batch_rows,
            exhausted,
        })
    }

    fn scan_search_rows_after_rowid(
        &self,
        plan: &SearchScanPlan,
        after_rowid: Option<i64>,
        row_limit: usize,
    ) -> Result<SearchRowBatch> {
        if row_limit == 0 {
            return Ok(SearchRowBatch {
                rows: Vec::new(),
                exhausted: true,
            });
        }

        let rowid_alias = plan
            .rowid_alias
            .as_deref()
            .expect("rowid cursor requires a rowid alias");
        let where_clause = match after_rowid {
            Some(_) if plan.where_clause.is_empty() => format!("WHERE {rowid_alias} > ?"),
            Some(_) => format!("{} AND {rowid_alias} > ?", plan.where_clause),
            None => plan.where_clause.clone(),
        };
        let fetch_limit = row_limit.saturating_add(1);
        let sql = format!(
            "SELECT {rowid_alias}, {select_list}
             FROM {safe_table_name}
             {where_clause}
             ORDER BY {rowid_alias} ASC
             LIMIT {fetch_limit}",
            select_list = plan.select_list,
            safe_table_name = plan.safe_table_name,
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let column_count = stmt.column_count().saturating_sub(1);
        let mut params = plan.filter_params.clone();
        if let Some(after_rowid) = after_rowid {
            params.push(Value::Integer(after_rowid));
        }
        let mut rows = stmt.query(params_from_iter(params.iter()))?;
        let mut batch_rows = Vec::with_capacity(fetch_limit.min(row_limit));
        while let Some(row) = rows.next()? {
            let rowid = row.get::<_, i64>(0)?;
            let mut values = Vec::with_capacity(column_count);
            for idx in 0..column_count {
                values.push(format_value(row.get_ref(idx + 1)?));
            }
            batch_rows.push(SearchRowBatchRow {
                rowid: Some(rowid),
                values,
            });
        }
        let exhausted = batch_rows.len() <= row_limit;
        if !exhausted {
            batch_rows.truncate(row_limit);
        }
        Ok(SearchRowBatch {
            rows: batch_rows,
            exhausted,
        })
    }
}

impl DeferredCurrentTableSearch {
    fn step(&mut self, db: &Database, row_limit: usize) -> Result<bool> {
        let batch = match self.cursor {
            DeferredSearchCursor::Offset(offset) => {
                db.scan_search_rows_chunk(&self.plan, offset, row_limit)?
            }
            DeferredSearchCursor::Rowid(after_rowid) => {
                db.scan_search_rows_after_rowid(&self.plan, after_rowid, row_limit)?
            }
        };
        for (index, row) in batch.rows.iter().enumerate() {
            let row_offset = self.next_row_offset + index;
            let summary = render_search_summary(&self.plan.columns, &row.values);
            let preview = render_search_preview(&self.plan.columns, &row.values);
            let summary_score = fuzzy_score(&summary, &self.query);
            let column_scores = row
                .values
                .iter()
                .map(|value| current_table_match_score(value, &self.query))
                .collect::<Vec<_>>();
            let matched_columns = column_scores
                .iter()
                .map(|score| score.is_some())
                .collect::<Vec<_>>();
            let score = column_scores
                .iter()
                .flatten()
                .copied()
                .max()
                .into_iter()
                .chain(summary_score)
                .max();
            if let Some(score) = score {
                let row_label = row
                    .rowid
                    .map(|rowid| format!("rowid {rowid}"))
                    .unwrap_or_else(|| format!("row {}", row_offset + 1));
                self.results.push(SearchHit {
                    table_name: self.plan.table_name.clone(),
                    rowid: row.rowid,
                    row_offset,
                    row_label,
                    values: row.values.clone(),
                    matched_columns,
                    haystack: preview,
                    score,
                });
                trim_current_table_hits(&mut self.results, self.limit);
            }
        }

        match &mut self.cursor {
            DeferredSearchCursor::Offset(offset) => *offset += batch.rows.len(),
            DeferredSearchCursor::Rowid(after_rowid) => {
                *after_rowid = batch.rows.last().and_then(|row| row.rowid);
            }
        }
        self.next_row_offset += batch.rows.len();
        if batch.exhausted {
            sort_current_table_hits(&mut self.results);
            self.results.truncate(self.limit);
            return Ok(true);
        }

        Ok(false)
    }
}

impl DeferredAllTablesSearch {
    fn step(&mut self, db: &Database, row_limit: usize) -> Result<bool> {
        if self.table_index >= self.tables.len() {
            sort_all_table_hits(&mut self.results);
            self.results.truncate(self.limit);
            return Ok(true);
        }

        let table = &self.tables[self.table_index];
        let batch = match self.cursor {
            DeferredSearchCursor::Offset(offset) => {
                db.scan_search_rows_chunk(&table.scan, offset, row_limit)?
            }
            DeferredSearchCursor::Rowid(after_rowid) => {
                db.scan_search_rows_after_rowid(&table.scan, after_rowid, row_limit)?
            }
        };
        for (index, row) in batch.rows.iter().enumerate() {
            let row_offset = self.next_row_offset + index;
            let summary = row.values.join(" | ");
            if let Some(score) = exact_match_score(&summary, &self.query_lower) {
                let row_label = row
                    .rowid
                    .map(|rowid| format!("rowid {rowid}"))
                    .unwrap_or_else(|| format!("row {}", row_offset + 1));
                self.results.push(SearchHit {
                    table_name: table.table_name.clone(),
                    rowid: row.rowid,
                    row_offset,
                    row_label,
                    values: row.values.clone(),
                    matched_columns: Vec::new(),
                    haystack: summary,
                    score,
                });
                trim_all_table_hits(&mut self.results, self.limit);
            }
        }

        match &mut self.cursor {
            DeferredSearchCursor::Offset(offset) => *offset += batch.rows.len(),
            DeferredSearchCursor::Rowid(after_rowid) => {
                *after_rowid = batch.rows.last().and_then(|row| row.rowid);
            }
        }
        self.next_row_offset += batch.rows.len();
        if batch.exhausted {
            self.table_index += 1;
            self.next_row_offset = 0;
            if self.table_index >= self.tables.len() {
                sort_all_table_hits(&mut self.results);
                self.results.truncate(self.limit);
                return Ok(true);
            }
            self.cursor = deferred_exact_table_cursor(&self.tables[self.table_index]);
        }

        Ok(false)
    }
}

fn deferred_current_table_cursor(plan: &SearchScanPlan) -> DeferredSearchCursor {
    if plan.rowid_alias.is_some() && plan.order_by.is_empty() {
        DeferredSearchCursor::Rowid(None)
    } else {
        DeferredSearchCursor::Offset(0)
    }
}

fn deferred_exact_table_cursor(plan: &ExactTableSearchPlan) -> DeferredSearchCursor {
    if plan.scan.rowid_alias.is_some() {
        DeferredSearchCursor::Rowid(None)
    } else {
        DeferredSearchCursor::Offset(0)
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

fn render_search_preview(columns: &[String], values: &[String]) -> String {
    columns
        .iter()
        .zip(values.iter())
        .map(|(column, value)| format!("{column}: {}", truncate_search_preview(value)))
        .collect::<Vec<_>>()
        .join(" | ")
}

fn truncate_search_preview(value: &str) -> String {
    const MAX_PREVIEW_CHARS: usize = 80;

    if value.chars().count() <= MAX_PREVIEW_CHARS {
        return value.to_string();
    }

    let truncated: String = value
        .chars()
        .take(MAX_PREVIEW_CHARS.saturating_sub(3))
        .collect();
    format!("{truncated}...")
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

fn trim_all_table_hits(results: &mut Vec<SearchHit>, limit: usize) {
    trim_search_hits(results, limit, sort_all_table_hits);
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
    let mut haystack_lower = Vec::new();
    let mut original_char_indexes = Vec::new();
    for (original_index, ch) in haystack.chars().enumerate() {
        for lower in ch.to_lowercase() {
            haystack_lower.push(lower);
            original_char_indexes.push(original_index);
        }
    }
    let query_lower: Vec<char> = query.chars().flat_map(char::to_lowercase).collect();

    if haystack_lower.is_empty() || query_lower.is_empty() {
        return Vec::new();
    }

    let mut best: Option<(i64, Vec<usize>)> = None;
    for (start_idx, candidate) in haystack_lower.iter().enumerate() {
        if *candidate != query_lower[0] {
            continue;
        }

        let mut positions = vec![original_char_indexes[start_idx]];
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
            positions.push(original_char_indexes[idx]);
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
