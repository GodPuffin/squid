use std::path::PathBuf;

use clap::Parser;

#[derive(Debug, Parser)]
#[command(author, version, about = "SQLite file viewer TUI built with ratatui")]
pub struct Cli {
    /// Path to a SQLite database file
    pub path: Option<PathBuf>,
}
