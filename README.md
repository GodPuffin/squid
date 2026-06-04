# squid 🦑

SQLite viewer and query runner for the terminal.

Open a `.db` or `.sqlite` file, browse tables and rows, inspect schema, search, sort, filter, view full row details, and switch into a writable SQL mode with syntax highlighting, history, completions, and result grids.

## Features

- **Browse** — table list, row preview, schema view, column visibility, multi-column sort, filters, and clipboard copy (`y`).
- **Search** — current-table live search (for smaller tables) and full-database search with jump-to-row.
- **SQL mode** — syntax highlighting, history, completions, and result grids for SELECT and write queries.
- **Row details** — edit fields, follow foreign keys, insert new rows (`a`), and delete rows (`d`) on writable tables.
- **Settings** — persistent preferences (`,` from home or browse): color scheme, session restore, recents, SQL limits, and more.
- **Themes** — dark, light, monokai, solarized (dark/light), and dracula.
- **Help** — press `?` anywhere for context-specific keybindings.

See [CHANGELOG.md](CHANGELOG.md) for release notes.

## Install

Windows:

```powershell
irm https://raw.githubusercontent.com/GodPuffin/squid/master/scripts/install.ps1 | iex
```

macOS / Linux:

```bash
curl -fsSL https://raw.githubusercontent.com/GodPuffin/squid/master/scripts/install.sh | sh
```

The installer downloads the latest GitHub release and adds `squid` to a user-local bin directory.

## Usage

```powershell
squid path\to\database.sqlite
```

Run without a path to open the home screen (recents and settings). With **Open last database on startup** enabled in settings, the most recent file opens automatically.

Copy to clipboard uses OSC 52 (works in many SSH terminals). In browse rows view, use `h` / `l` to select a column, then `y` to copy the cell. In the detail modal or SQL editor, `y` copies the current field or full query.

Build from source:

```powershell
git clone https://github.com/GodPuffin/squid
cd squid
cargo build --release
.\target\release\squid.exe path\to\database.sqlite
```
