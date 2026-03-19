# squid 🦑

SQLite viewer and query runner for the terminal.

Open a `.db` or `.sqlite` file, browse tables and rows, inspect schema, search, sort, filter, view full row details, and switch into a writable SQL mode with syntax highlighting, history, completions, and result grids.

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

Build from source:

```powershell
git clone https://github.com/GodPuffin/squid
cd squid
cargo build --release
.\target\release\squid.exe path\to\database.sqlite
```

## Modes

- `1` switches to `Browse` mode.
- `2` switches to `SQL` mode.
- In SQL mode, use `F5` to execute the current query and `F2` to open completions.
