use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use ratatui::style::{Color, Modifier, Style};
use rusqlite::Connection;

use crate::app::App;

#[test]
fn schema_display_lines_highlight_create_sql_snapshot() {
    let app = app_with_schema("content-schema-highlight");
    let lines = super::schema_display_lines(&app);
    let header_index = lines
        .iter()
        .position(|line| line.spans.len() == 1 && line.spans[0].content.as_ref() == "Create SQL")
        .expect("create sql header");
    let sql_lines = &lines[header_index + 1..];

    assert!(!sql_lines.is_empty());
    assert!(sql_lines.iter().any(|line| {
        line.spans.iter().any(|span| {
            span.content.as_ref() == "CREATE"
                && span.style
                    == Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
        })
    }));
    assert_eq!(lines[0].spans.len(), 1);
    assert_eq!(lines[0].spans[0].content.as_ref(), "Table: demo");
    assert_eq!(lines[0].spans[0].style, Style::default());
}

fn app_with_schema(label: &str) -> App {
    let path = temp_db_path(label);
    let conn = Connection::open(&path).expect("create db");
    conn.execute_batch(
        "CREATE TABLE demo(
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL DEFAULT 'alpha'
        );",
    )
    .expect("seed db");
    drop(conn);

    let app = App::load(path.clone()).expect("load app");
    let _ = fs::remove_file(path);
    app
}

fn temp_db_path(label: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("squid-content-{label}-{stamp}.sqlite"))
}
