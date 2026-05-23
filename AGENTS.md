# AGENTS.md

## Cursor Cloud specific instructions

### Overview

**squid** is a standalone Rust TUI application for browsing SQLite databases in the terminal. It uses `ratatui` for rendering and `rusqlite` (with the `bundled` feature, so no system SQLite needed).

### Prerequisites

- Rust stable toolchain **1.85+** (the crate uses `edition = "2024"`)
- A C compiler (`cc`/`gcc`) is needed to compile the bundled SQLite C source via the `libsqlite3-sys` crate

### Common commands

| Task | Command |
|------|---------|
| Build (debug) | `cargo build` |
| Build (release) | `cargo build --release` |
| Lint (format) | `cargo fmt --check` |
| Lint (clippy) | `cargo clippy` |
| Test | `cargo test` |
| Run | `cargo run -- path/to/database.db` |

### Notes

- The project has no external service dependencies (no Docker, no databases to run, no network services).
- `rusqlite` uses the `bundled` feature so it compiles its own SQLite — no system `libsqlite3-dev` package required.
- To manually test the TUI you need a SQLite file. Create one with: `sqlite3 /tmp/test.db "CREATE TABLE t(id INTEGER PRIMARY KEY, val TEXT); INSERT INTO t VALUES(1,'hello');"` then run `cargo run -- /tmp/test.db`.
- The TUI requires a TTY; running inside `tmux` works well for automated testing of the interactive application.
