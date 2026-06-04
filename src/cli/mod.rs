use std::path::PathBuf;

use clap::Parser;

#[derive(Debug, Default, Parser)]
#[command(author, version, about = "SQLite file viewer TUI built with ratatui")]
pub struct Cli {
    /// Path to a SQLite database file
    pub path: Option<PathBuf>,

    /// Open the database read-only (fail if read-only open is not possible)
    #[arg(long)]
    pub readonly: bool,

    /// Initial color scheme (dark, light, monokai, solarized_dark, solarized_light, dracula)
    #[arg(long, value_name = "NAME")]
    pub scheme: Option<String>,

    /// Skip restoring and saving per-database session state
    #[arg(long)]
    pub no_session: bool,
}

impl From<Option<PathBuf>> for Cli {
    fn from(path: Option<PathBuf>) -> Self {
        Self {
            path,
            ..Self::default()
        }
    }
}

impl From<PathBuf> for Cli {
    fn from(path: PathBuf) -> Self {
        Self {
            path: Some(path),
            ..Self::default()
        }
    }
}

#[cfg(test)]
mod tests;
