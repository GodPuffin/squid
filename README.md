# squid

Read-only SQLite viewer TUI built with `ratatui`.

`squid` is for opening a `.db` / `.sqlite` file in the terminal, browsing tables, previewing rows, inspecting schema, filtering and sorting rows, searching, and opening full row details without writing to the database.

## Features

- Read-only SQLite access
- Table list and row preview
- Schema view with column metadata and `CREATE TABLE` SQL
- Row detail modal with full cell values
- Foreign-key jump from row details
- Per-table column visibility
- Per-table multi-column sort
- Per-table row filters
- Current-table fuzzy search
- All-table exact search
- Keyboard and mouse support

## Install

### Today: build from source

You need Rust installed.

```powershell
git clone <your-repo-url>
cd squid
cargo build --release
```

The binary will be at:

```text
target\release\squid.exe
```

Run it with:

```powershell
.\target\release\squid.exe path\to\database.sqlite
```

### End-user install without Rust

Yes, this is possible.

The usual approach is:

1. Build release binaries in CI for Windows/macOS/Linux.
2. Attach those binaries to GitHub Releases.
3. Ship a small installer script that downloads the latest release, places `squid` in a user bin directory, and adds that directory to `PATH`.

That gives you a true one-command install for someone who just wants the tool, without requiring Rust.

On Windows, the target experience would look like:

```powershell
irm https://<your-domain-or-github-raw>/install.ps1 | iex
```

What that installer would do:

- download the latest prebuilt `squid.exe`
- install it to something like `%USERPROFILE%\.local\bin`
- add that directory to the user `PATH` if needed

Once GitHub Releases are being published, the intended install commands are:

Windows:

```powershell
irm https://raw.githubusercontent.com/GodPuffin/squid/main/scripts/install.ps1 | iex
```

macOS / Linux:

```bash
curl -fsSL https://raw.githubusercontent.com/GodPuffin/squid/main/scripts/install.sh | sh
```

These scripts install the latest release binary into a user-local bin directory and update `PATH`.

## Usage

```powershell
cargo run -- path\to\database.sqlite
```

Or with the release binary:

```powershell
squid path\to\database.sqlite
```

## Main Controls

- `q`: quit
- `Tab` or Left / Right: change focus or active pane
- Up / Down: move selection
- `v`: toggle rows / schema
- `Enter` on a row: open row details
- double-click a row: open row details
- `m`: open view/sort modal
- `M`: open filters modal
- `f`: fuzzy search current table
- `F`: search all tables
- `r`: reload database metadata and preview

## Search

- `f` opens live fuzzy search on the current table
- `F` opens all-table search
- in all-table search, `Enter` submits the query
- `Enter` on a result jumps to that row
- double-clicking a search result also jumps

## Row Details

- shows every value in the selected row without table-column truncation
- `g` follows the selected foreign-key reference when one is available
- `Esc` closes the detail modal

## Sorting and Filtering

### View / sort modal

Open with `m`.

- hide/show columns
- build multi-column sort priority

### Filter modal

Open with `M`.

- text columns: `contains`, `equals`, `starts with`
- numeric columns: `equals`, `greater than`, `less than`
- boolean columns: `is true`, `is false`

Applied filters are shown in the main content title so filtered views are obvious after leaving the modal.

## Developer Guide

### Requirements

- Rust toolchain
- Windows terminal with mouse support if you want the full mouse UI

You do not need a separate SQLite install for the app itself because `rusqlite` is built with the `bundled` feature.

### Common commands

```powershell
cargo fmt --check
cargo check
cargo build
```

### CI and releases

- `.github/workflows/ci.yml`: runs on pushes to `main` and on pull requests
- `.github/workflows/release.yml`: runs only when you push a version tag like `v0.1.0`

Release flow:

```powershell
git tag v0.1.0
git push origin v0.1.0
```

That tag triggers:

- release builds for Windows, Linux, and macOS
- GitHub Release creation
- GitHub auto-generated release notes
- upload of platform archives used by the installer scripts

Run against a sample database:

```powershell
cargo run -- .\sakila.db
```

### Project layout

```text
src/
  main.rs          event loop and input dispatch
  db.rs            SQLite access and query shaping
  app.rs           shared app types/state surface
  app/
    core.rs        table/row navigation and refresh
    detail.rs      row detail modal behavior
    filter.rs      filter modal behavior
    modal.rs       view/sort modal behavior
    search.rs      search behavior
  ui.rs            top-level rendering entry
  ui/
    layout.rs      pane sizing and hit-testing
    panels.rs      main panes
    detail.rs      row detail modal UI
    filter.rs      filter modal UI
    modal.rs       view/sort modal UI
    search.rs      search UI
```

### Notes

- The app is intended to stay read-only.
- BLOB values are shown as byte counts, not raw binary content.
- Some navigation paths rely on SQLite `rowid`; `WITHOUT ROWID` tables have more limited jump behavior.
