mod app;
mod db;
mod runtime;
mod ui;

use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;

#[derive(Debug, Parser)]
#[command(author, version, about = "SQLite file viewer TUI built with ratatui")]
struct Cli {
    /// Path to a SQLite database file
    path: Option<PathBuf>,
}

fn main() -> Result<()> {
    runtime::run(Cli::parse().path)
}

#[cfg(test)]
#[path = "testing/main_cli.rs"]
mod tests;
