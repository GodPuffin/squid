use anyhow::Result;

use crate::db::SqlExecutionResult;

use super::{
    Action, App, SqlCompletionItem, SqlCompletionState, SqlHistoryEntry, SqlPane, SqlResultState,
};

const SQL_RESULT_LIMIT: usize = 200;
const SQL_KEYWORDS: &[&str] = &[
    "SELECT",
    "FROM",
    "WHERE",
    "ORDER BY",
    "GROUP BY",
    "LIMIT",
    "INSERT INTO",
    "VALUES",
    "UPDATE",
    "SET",
    "DELETE FROM",
    "CREATE TABLE",
    "ALTER TABLE",
    "DROP TABLE",
    "JOIN",
    "LEFT JOIN",
    "INNER JOIN",
    "PRAGMA",
];

const SQL_SNIPPETS: &[(&str, &str)] = &[
    ("SELECT * FROM", "SELECT *\nFROM "),
    ("SELECT WHERE", "SELECT *\nFROM \nWHERE "),
    ("INSERT INTO", "INSERT INTO  ()\nVALUES ();"),
    ("UPDATE SET", "UPDATE \nSET \nWHERE ;"),
    ("DELETE FROM", "DELETE FROM \nWHERE ;"),
];

impl App {
    pub fn set_sql_viewport_sizes(
        &mut self,
        editor_height: usize,
        history_height: usize,
        result_height: usize,
    ) {
        self.sql.editor_height = editor_height.max(1);
        self.sql.history_height = history_height.max(1);
        self.sql.result_height = result_height.max(1);
        self.ensure_sql_viewport();
    }

    pub fn sql_focus(&self) -> SqlPane {
        self.sql.focus
    }

    pub fn sql_query_lines(&self) -> Vec<String> {
        if self.sql.query.is_empty() {
            vec![String::new()]
        } else {
            self.sql.query.lines().map(str::to_string).collect()
        }
    }

    pub fn sql_visible_history(&self) -> &[SqlHistoryEntry] {
        let len = self.sql.history.len();
        let end = (self.sql.history_offset + self.sql.history_height).min(len);
        &self.sql.history[self.sql.history_offset.min(len)..end]
    }

    pub fn sql_selected_history_in_view(&self) -> Option<usize> {
        if self.sql.history.is_empty() {
            None
        } else {
            self.sql
                .selected_history
                .checked_sub(self.sql.history_offset)
                .filter(|index| *index < self.sql_visible_history().len())
        }
    }

    pub fn sql_result_rows_in_view(&self) -> &[Vec<String>] {
        match &self.sql.result {
            SqlResultState::Rows { rows, .. } => {
                let len = rows.len();
                let end = (self.sql.result_scroll + self.sql.result_height).min(len);
                &rows[self.sql.result_scroll.min(len)..end]
            }
            SqlResultState::Empty | SqlResultState::Message { .. } => &[],
        }
    }

    pub fn sql_result_columns(&self) -> &[String] {
        match &self.sql.result {
            SqlResultState::Rows { columns, .. } => columns,
            SqlResultState::Empty | SqlResultState::Message { .. } => &[],
        }
    }

    pub fn sql_completion_items(&self) -> &[SqlCompletionItem] {
        self.sql
            .completion
            .as_ref()
            .map(|completion| completion.items.as_slice())
            .unwrap_or(&[])
    }

    pub fn sql_selected_completion_in_view(&self) -> Option<usize> {
        self.sql.completion.as_ref().and_then(|completion| {
            (!completion.items.is_empty())
                .then_some(completion.selected.min(completion.items.len() - 1))
        })
    }

    pub fn sql_cursor_line_col(&self) -> (usize, usize) {
        line_col_from_index(&self.sql.query, self.sql.cursor)
    }

    pub fn sql_select_history_in_view(&mut self, index: usize) {
        let absolute = self.sql.history_offset + index;
        if absolute < self.sql.history.len() {
            self.sql.selected_history = absolute;
            self.sql.focus = SqlPane::History;
            self.ensure_sql_viewport();
        }
    }

    pub fn sql_set_cursor_from_view(&mut self, line_in_view: usize, col: usize) {
        let line = self.sql.editor_scroll + line_in_view;
        let target_col = col.saturating_sub(1);
        self.sql.cursor = index_for_line_col(&self.sql.query, line, target_col);
        self.sql.focus = SqlPane::Editor;
        let _ = self.sql_refresh_completion();
        self.ensure_sql_viewport();
    }

    pub fn sql_select_completion_in_view(&mut self, index: usize) {
        if let Some(completion) = &mut self.sql.completion
            && index < completion.items.len()
        {
            completion.selected = index;
            self.sql.focus = SqlPane::Editor;
        }
    }

    pub fn sql_apply_selected_completion(&mut self) {
        self.sql_apply_completion();
    }

    pub fn sql_focus_editor(&mut self) {
        self.sql.focus = SqlPane::Editor;
    }

    pub fn sql_focus_results(&mut self) {
        self.sql.focus = SqlPane::Results;
    }

    pub(super) fn ensure_sql_viewport(&mut self) {
        let (line, _) = self.sql_cursor_line_col();
        let max_editor_scroll = self
            .sql_query_lines()
            .len()
            .saturating_sub(self.sql.editor_height);
        self.sql.editor_scroll = self.sql.editor_scroll.min(max_editor_scroll);
        if line < self.sql.editor_scroll {
            self.sql.editor_scroll = line;
        }
        if line >= self.sql.editor_scroll + self.sql.editor_height {
            self.sql.editor_scroll = line + 1 - self.sql.editor_height;
        }

        let max_history_offset = self
            .sql
            .history
            .len()
            .saturating_sub(self.sql.history_height);
        self.sql.history_offset = self.sql.history_offset.min(max_history_offset);
        if self.sql.selected_history < self.sql.history_offset {
            self.sql.history_offset = self.sql.selected_history;
        }
        if self.sql.selected_history >= self.sql.history_offset + self.sql.history_height {
            self.sql.history_offset = self.sql.selected_history + 1 - self.sql.history_height;
        }

        if let SqlResultState::Rows { rows, .. } = &self.sql.result {
            let max_result_scroll = rows.len().saturating_sub(self.sql.result_height);
            self.sql.result_scroll = self.sql.result_scroll.min(max_result_scroll);
        } else {
            self.sql.result_scroll = 0;
        }
    }

    pub(super) fn handle_sql(&mut self, action: Action) -> Result<()> {
        match action {
            Action::None => {}
            Action::ToggleFocus => self.sql_cycle_focus(),
            Action::ReverseFocus => self.sql_cycle_focus_back(),
            Action::MoveUp => self.sql_move_up(),
            Action::MoveDown => self.sql_move_down(),
            Action::MoveLeft => self.sql_move_left(),
            Action::MoveRight => self.sql_move_right(),
            Action::MoveHome => self.sql_move_home(),
            Action::MoveEnd => self.sql_move_end(),
            Action::PageUp => self.sql_page_up(),
            Action::PageDown => self.sql_page_down(),
            Action::InputChar(ch) => self.sql_insert_char(ch)?,
            Action::Backspace => self.sql_backspace()?,
            Action::Delete => self.sql_delete()?,
            Action::NewLine => self.sql_newline()?,
            Action::ExecuteSql => self.sql_execute()?,
            Action::Confirm => self.sql_confirm()?,
            Action::Clear => self.sql_clear(),
            Action::Reload => self.reload()?,
            Action::CloseModal => self.sql.completion = None,
            Action::Quit => {}
            Action::SwitchToBrowse
            | Action::SwitchToSql
            | Action::ToggleView
            | Action::OpenConfig
            | Action::ToggleItem
            | Action::FollowLink
            | Action::OpenSearchCurrent
            | Action::OpenSearchAll
            | Action::OpenFilters => {}
        }

        Ok(())
    }

    fn sql_cycle_focus(&mut self) {
        self.sql.focus = match self.sql.focus {
            SqlPane::Editor => SqlPane::History,
            SqlPane::History => SqlPane::Results,
            SqlPane::Results => SqlPane::Editor,
        };
        self.sql.completion = None;
    }

    fn sql_cycle_focus_back(&mut self) {
        self.sql.focus = match self.sql.focus {
            SqlPane::Editor => SqlPane::Results,
            SqlPane::History => SqlPane::Editor,
            SqlPane::Results => SqlPane::History,
        };
        self.sql.completion = None;
    }

    fn sql_move_up(&mut self) {
        if self.sql.focus == SqlPane::Editor
            && let Some(completion) = &mut self.sql.completion
            && completion.selected > 0
        {
            completion.selected -= 1;
            return;
        }

        match self.sql.focus {
            SqlPane::Editor => {
                self.sql.cursor = move_vertical(&self.sql.query, self.sql.cursor, -1);
                self.ensure_sql_viewport();
            }
            SqlPane::History => {
                if self.sql.selected_history > 0 {
                    self.sql.selected_history -= 1;
                    self.ensure_sql_viewport();
                }
            }
            SqlPane::Results => {
                if self.sql.result_scroll > 0 {
                    self.sql.result_scroll -= 1;
                }
            }
        }
    }

    fn sql_move_down(&mut self) {
        if self.sql.focus == SqlPane::Editor
            && let Some(completion) = &mut self.sql.completion
            && completion.selected + 1 < completion.items.len()
        {
            completion.selected += 1;
            return;
        }

        match self.sql.focus {
            SqlPane::Editor => {
                self.sql.cursor = move_vertical(&self.sql.query, self.sql.cursor, 1);
                self.ensure_sql_viewport();
            }
            SqlPane::History => {
                if self.sql.selected_history + 1 < self.sql.history.len() {
                    self.sql.selected_history += 1;
                    self.ensure_sql_viewport();
                }
            }
            SqlPane::Results => {
                if let SqlResultState::Rows { rows, .. } = &self.sql.result
                    && self.sql.result_scroll + self.sql.result_height < rows.len()
                {
                    self.sql.result_scroll += 1;
                }
            }
        }
    }

    fn sql_move_left(&mut self) {
        if self.sql.focus == SqlPane::Editor {
            self.sql.cursor = previous_boundary(&self.sql.query, self.sql.cursor);
            let _ = self.sql_refresh_completion();
            self.ensure_sql_viewport();
        }
    }

    fn sql_move_right(&mut self) {
        if self.sql.focus == SqlPane::Editor {
            self.sql.cursor = next_boundary(&self.sql.query, self.sql.cursor);
            let _ = self.sql_refresh_completion();
            self.ensure_sql_viewport();
        }
    }

    fn sql_move_home(&mut self) {
        match self.sql.focus {
            SqlPane::Editor => {
                let (line, _) = line_col_from_index(&self.sql.query, self.sql.cursor);
                self.sql.cursor = index_for_line_col(&self.sql.query, line, 0);
                let _ = self.sql_refresh_completion();
                self.ensure_sql_viewport();
            }
            SqlPane::History => {
                self.sql.selected_history = 0;
                self.ensure_sql_viewport();
            }
            SqlPane::Results => {
                self.sql.result_scroll = 0;
            }
        }
    }

    fn sql_move_end(&mut self) {
        match self.sql.focus {
            SqlPane::Editor => {
                let (line, _) = line_col_from_index(&self.sql.query, self.sql.cursor);
                let len = line_length(&self.sql.query, line);
                self.sql.cursor = index_for_line_col(&self.sql.query, line, len);
                let _ = self.sql_refresh_completion();
                self.ensure_sql_viewport();
            }
            SqlPane::History => {
                if !self.sql.history.is_empty() {
                    self.sql.selected_history = self.sql.history.len() - 1;
                    self.ensure_sql_viewport();
                }
            }
            SqlPane::Results => {
                if let SqlResultState::Rows { rows, .. } = &self.sql.result {
                    self.sql.result_scroll = rows.len().saturating_sub(self.sql.result_height);
                }
            }
        }
    }

    fn sql_page_up(&mut self) {
        match self.sql.focus {
            SqlPane::Editor => {
                for _ in 0..self.sql.editor_height {
                    self.sql.cursor = move_vertical(&self.sql.query, self.sql.cursor, -1);
                }
                let _ = self.sql_refresh_completion();
                self.ensure_sql_viewport();
            }
            SqlPane::History => {
                self.sql.selected_history = self
                    .sql
                    .selected_history
                    .saturating_sub(self.sql.history_height);
                self.ensure_sql_viewport();
            }
            SqlPane::Results => {
                self.sql.result_scroll = self
                    .sql
                    .result_scroll
                    .saturating_sub(self.sql.result_height);
            }
        }
    }

    fn sql_page_down(&mut self) {
        match self.sql.focus {
            SqlPane::Editor => {
                for _ in 0..self.sql.editor_height {
                    self.sql.cursor = move_vertical(&self.sql.query, self.sql.cursor, 1);
                }
                let _ = self.sql_refresh_completion();
                self.ensure_sql_viewport();
            }
            SqlPane::History => {
                if !self.sql.history.is_empty() {
                    self.sql.selected_history = (self.sql.selected_history
                        + self.sql.history_height)
                        .min(self.sql.history.len() - 1);
                    self.ensure_sql_viewport();
                }
            }
            SqlPane::Results => {
                if let SqlResultState::Rows { rows, .. } = &self.sql.result {
                    self.sql.result_scroll = (self.sql.result_scroll + self.sql.result_height)
                        .min(rows.len().saturating_sub(self.sql.result_height));
                }
            }
        }
    }

    fn sql_insert_char(&mut self, ch: char) -> Result<()> {
        if self.sql.focus != SqlPane::Editor {
            return Ok(());
        }
        self.sql.query.insert(self.sql.cursor, ch);
        self.sql.cursor += ch.len_utf8();
        self.sql_refresh_completion()?;
        self.ensure_sql_viewport();
        Ok(())
    }

    fn sql_backspace(&mut self) -> Result<()> {
        if self.sql.focus != SqlPane::Editor || self.sql.cursor == 0 {
            return Ok(());
        }
        let start = previous_boundary(&self.sql.query, self.sql.cursor);
        self.sql.query.replace_range(start..self.sql.cursor, "");
        self.sql.cursor = start;
        self.sql_refresh_completion()?;
        self.ensure_sql_viewport();
        Ok(())
    }

    fn sql_delete(&mut self) -> Result<()> {
        if self.sql.focus != SqlPane::Editor || self.sql.cursor >= self.sql.query.len() {
            return Ok(());
        }
        let end = next_boundary(&self.sql.query, self.sql.cursor);
        self.sql.query.replace_range(self.sql.cursor..end, "");
        self.sql_refresh_completion()?;
        self.ensure_sql_viewport();
        Ok(())
    }

    fn sql_newline(&mut self) -> Result<()> {
        match self.sql.focus {
            SqlPane::Editor => {
                if self.sql.completion.is_some() {
                    self.sql_apply_completion();
                    return Ok(());
                }
                self.sql_insert_char('\n')?;
            }
            SqlPane::History => self.sql_load_history_selected(),
            SqlPane::Results => {}
        }
        Ok(())
    }

    fn sql_confirm(&mut self) -> Result<()> {
        match self.sql.focus {
            SqlPane::Editor => {
                if self.sql.completion.is_some() {
                    self.sql_apply_completion();
                } else {
                    self.sql_refresh_completion()?;
                }
            }
            SqlPane::History => self.sql_load_history_selected(),
            SqlPane::Results => {}
        }
        Ok(())
    }

    fn sql_clear(&mut self) {
        match self.sql.focus {
            SqlPane::Editor => {
                self.sql.query.clear();
                self.sql.cursor = 0;
                self.sql.editor_scroll = 0;
                self.sql.completion = None;
                self.sql.status = "Query cleared".to_string();
            }
            SqlPane::History => {
                self.sql.history.clear();
                self.sql.history_offset = 0;
                self.sql.selected_history = 0;
                self.sql.status = "History cleared".to_string();
            }
            SqlPane::Results => {
                self.sql.result = SqlResultState::Empty;
                self.sql.result_scroll = 0;
                self.sql.status = "Results cleared".to_string();
            }
        }
    }

    fn sql_execute(&mut self) -> Result<()> {
        let query = self.sql.query.trim().to_string();
        if query.is_empty() {
            self.sql.result = SqlResultState::Message {
                text: "Query is empty".to_string(),
                is_error: true,
            };
            self.sql.status = "Execution failed".to_string();
            return Ok(());
        }

        match self.db.execute_sql(&query, SQL_RESULT_LIMIT) {
            Ok(SqlExecutionResult::Rows {
                columns,
                rows,
                is_mutation,
            }) => {
                let row_count = rows.len();
                self.sql.result = SqlResultState::Rows { columns, rows };
                self.sql.result_scroll = 0;
                self.sql.status = format!("Returned {row_count} row(s)");
                self.push_sql_history(query, format!("Rows: {row_count}"));
                if is_mutation {
                    self.refresh_loaded_db_state()?;
                }
            }
            Ok(SqlExecutionResult::Statement {
                affected_rows,
                description,
            }) => {
                let text = format!("{description} ok ({affected_rows} row(s) affected)");
                self.sql.result = SqlResultState::Message {
                    text: text.clone(),
                    is_error: false,
                };
                self.sql.result_scroll = 0;
                self.sql.status = text.clone();
                self.push_sql_history(query, text);
                self.refresh_loaded_db_state()?;
            }
            Err(err) => {
                let text = err.to_string();
                self.sql.result = SqlResultState::Message {
                    text: text.clone(),
                    is_error: true,
                };
                self.sql.status = "Execution failed".to_string();
                self.push_sql_history(query, format!("Error: {text}"));
            }
        }

        self.sql.focus = SqlPane::Results;
        self.sql.completion = None;
        self.ensure_sql_viewport();
        Ok(())
    }

    fn push_sql_history(&mut self, query: String, summary: String) {
        if self
            .sql
            .history
            .last()
            .is_some_and(|entry| entry.query == query)
        {
            if let Some(last) = self.sql.history.last_mut() {
                last.summary = summary;
            }
        } else {
            self.sql.history.push(SqlHistoryEntry {
                query: query.clone(),
                summary,
            });
        }
        if !self.sql.history.is_empty() {
            self.sql.selected_history = self.sql.history.len() - 1;
        }
        self.ensure_sql_viewport();
    }

    fn sql_load_history_selected(&mut self) {
        if let Some(entry) = self.sql.history.get(self.sql.selected_history) {
            self.sql.query = entry.query.clone();
            self.sql.cursor = self.sql.query.len();
            self.sql.focus = SqlPane::Editor;
            self.sql.completion = None;
            self.ensure_sql_viewport();
        }
    }

    fn sql_refresh_completion(&mut self) -> Result<()> {
        if self.sql.focus != SqlPane::Editor {
            return Ok(());
        }

        let (prefix_start, prefix) = completion_prefix(&self.sql.query, self.sql.cursor);
        if prefix.is_empty() {
            self.sql.completion = None;
            return Ok(());
        }
        let items = self.sql_completion_candidates(&prefix)?;
        self.sql.completion = (!items.is_empty()).then_some(SqlCompletionState {
            prefix_start,
            selected: 0,
            items,
        });
        Ok(())
    }

    fn sql_apply_completion(&mut self) {
        let Some(completion) = &self.sql.completion else {
            return;
        };
        let Some(item) = completion.items.get(completion.selected) else {
            return;
        };

        let end = self.sql.cursor;
        self.sql
            .query
            .replace_range(completion.prefix_start..end, &item.insert_text);
        self.sql.cursor = completion.prefix_start + item.insert_text.len();
        self.sql.completion = None;
        self.ensure_sql_viewport();
    }

    fn sql_completion_candidates(&self, prefix: &str) -> Result<Vec<SqlCompletionItem>> {
        let prefix_lower = prefix.to_lowercase();
        let qualifier = completion_qualifier(prefix);
        let mut items = Vec::new();

        for keyword in SQL_KEYWORDS {
            items.push(SqlCompletionItem {
                label: (*keyword).to_string(),
                insert_text: (*keyword).to_string(),
            });
        }

        for (label, insert_text) in SQL_SNIPPETS {
            items.push(SqlCompletionItem {
                label: (*label).to_string(),
                insert_text: (*insert_text).to_string(),
            });
        }

        for table in &self.tables {
            items.push(SqlCompletionItem {
                label: table.name.clone(),
                insert_text: table.name.clone(),
            });

            for column in self.db.list_columns(&table.name)? {
                items.push(SqlCompletionItem {
                    label: format!("{}.{}", table.name, column),
                    insert_text: format!("{qualifier}{column}"),
                });
            }
        }

        items.sort_by(|left, right| left.label.cmp(&right.label));
        items.dedup_by(|left, right| left.label.eq_ignore_ascii_case(&right.label));

        if prefix_lower.is_empty() {
            Ok(items.into_iter().take(6).collect())
        } else {
            Ok(items
                .into_iter()
                .filter(|item| item.label.to_lowercase().starts_with(&prefix_lower))
                .take(6)
                .collect())
        }
    }
}

fn previous_boundary(value: &str, index: usize) -> usize {
    value[..index]
        .char_indices()
        .last()
        .map(|(idx, _)| idx)
        .unwrap_or(0)
}

fn next_boundary(value: &str, index: usize) -> usize {
    value[index..]
        .char_indices()
        .nth(1)
        .map(|(offset, _)| index + offset)
        .unwrap_or_else(|| value.len())
}

fn line_col_from_index(value: &str, index: usize) -> (usize, usize) {
    let mut line = 0;
    let mut col = 0;
    for (byte_idx, ch) in value.char_indices() {
        if byte_idx >= index {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    (line, col)
}

fn index_for_line_col(value: &str, target_line: usize, target_col: usize) -> usize {
    let mut line = 0;
    let mut col = 0;
    for (idx, ch) in value.char_indices() {
        if line == target_line && col == target_col {
            return idx;
        }
        if ch == '\n' {
            if line == target_line {
                return idx;
            }
            line += 1;
            col = 0;
        } else if line == target_line {
            col += 1;
        }
    }
    value.len()
}

fn line_length(value: &str, target_line: usize) -> usize {
    value
        .lines()
        .nth(target_line)
        .map(|line| line.chars().count())
        .unwrap_or(0)
}

fn move_vertical(value: &str, index: usize, delta: isize) -> usize {
    let (line, col) = line_col_from_index(value, index);
    let target_line = if delta.is_negative() {
        line.saturating_sub(delta.unsigned_abs())
    } else {
        line.saturating_add(delta as usize)
    };
    let target_col = col.min(line_length(value, target_line));
    index_for_line_col(value, target_line, target_col)
}

fn completion_prefix(value: &str, cursor: usize) -> (usize, String) {
    let mut start = cursor;
    while start > 0 {
        let previous = previous_boundary(value, start);
        let ch = value[previous..start].chars().next().unwrap_or(' ');
        if is_completion_char(ch) {
            start = previous;
        } else {
            break;
        }
    }
    (start, value[start..cursor].to_string())
}

fn completion_qualifier(prefix: &str) -> &str {
    prefix
        .rfind('.')
        .map(|index| &prefix[..=index])
        .unwrap_or("")
}

fn is_completion_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_' || ch == '.'
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use rusqlite::Connection;

    use super::{completion_prefix, completion_qualifier, line_col_from_index, move_vertical};
    use crate::app::App;

    #[test]
    fn completion_prefix_reads_identifier_prefix() {
        let query = "SELECT ac";
        let (start, prefix) = completion_prefix(query, query.len());
        assert_eq!(start, 7);
        assert_eq!(prefix, "ac");
    }

    #[test]
    fn line_column_round_trips() {
        let query = "SELECT\nname";
        assert_eq!(line_col_from_index(query, 0), (0, 0));
        assert_eq!(line_col_from_index(query, 7), (1, 0));
    }

    #[test]
    fn vertical_movement_preserves_column_when_possible() {
        let query = "SELECT\ncolumn\nx";
        let moved = move_vertical(query, query.len() - 1, -1);
        assert_eq!(line_col_from_index(query, moved), (1, 0));
    }

    #[test]
    fn completion_qualifier_keeps_table_or_alias_prefix() {
        assert_eq!(completion_qualifier("orders."), "orders.");
        assert_eq!(completion_qualifier("o.id"), "o.");
        assert_eq!(completion_qualifier("id"), "");
    }

    #[test]
    fn sql_completion_preserves_qualified_prefix_when_applied() {
        let path = temp_db_path("qualified-completion");
        let conn = Connection::open(&path).expect("create db");
        conn.execute("CREATE TABLE orders(id INTEGER PRIMARY KEY, name TEXT)", [])
            .expect("create table");
        drop(conn);

        let mut app = App::load(path.clone()).expect("load app");
        app.sql.query = "SELECT orders.".to_string();
        app.sql.cursor = app.sql.query.len();
        app.sql_refresh_completion().expect("refresh completion");
        let completion = app.sql.completion.as_mut().expect("completion");
        completion.selected = completion
            .items
            .iter()
            .position(|item| item.label == "orders.id")
            .expect("orders.id completion");

        app.sql_apply_completion();

        assert_eq!(app.sql.query, "SELECT orders.id");

        let _ = fs::remove_file(path);
    }

    #[test]
    fn sql_execute_reloads_after_insert_returning() {
        let path = temp_db_path("insert-returning");
        let conn = Connection::open(&path).expect("create db");
        conn.execute("CREATE TABLE demo(id INTEGER PRIMARY KEY, name TEXT)", [])
            .expect("create table");
        drop(conn);

        let mut app = App::load(path.clone()).expect("load app");
        assert_eq!(app.preview.total_rows, 0);

        app.sql.query = "INSERT INTO demo(name) VALUES ('delta') RETURNING id".to_string();
        app.sql.cursor = app.sql.query.len();
        app.sql_execute().expect("execute sql");

        assert_eq!(app.preview.total_rows, 1);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn sql_execute_preserves_connection_scoped_state() {
        let path = temp_db_path("connection-state");
        let conn = Connection::open(&path).expect("create db");
        conn.execute("CREATE TABLE demo(id INTEGER PRIMARY KEY, name TEXT)", [])
            .expect("create table");
        drop(conn);

        let mut app = App::load(path.clone()).expect("load app");

        app.sql.query = "CREATE TEMP TABLE temp_demo(value TEXT)".to_string();
        app.sql.cursor = app.sql.query.len();
        app.sql_execute().expect("create temp table");

        app.sql.query = "INSERT INTO temp_demo(value) VALUES ('kept')".to_string();
        app.sql.cursor = app.sql.query.len();
        app.sql_execute().expect("insert temp row");

        app.sql.query = "SELECT value FROM temp_demo".to_string();
        app.sql.cursor = app.sql.query.len();
        app.sql_execute().expect("select temp row");

        match &app.sql.result {
            crate::app::SqlResultState::Rows { columns, rows } => {
                assert_eq!(columns, &vec!["value".to_string()]);
                assert_eq!(rows, &vec![vec!["kept".to_string()]]);
            }
            result => panic!("expected rows, got {result:?}"),
        }

        let _ = fs::remove_file(path);
    }

    fn temp_db_path(label: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("squid-sql-{label}-{stamp}.sqlite"))
    }
}
