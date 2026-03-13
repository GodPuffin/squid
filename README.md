# squid

Read-only SQLite viewer for the terminal.

Open a `.db` or `.sqlite` file, browse tables and rows, inspect schema, search, sort, filter, and view full row details without writing to the database.

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

## Controls

- `q`: quit
- `Tab` or Left / Right: switch pane
- Up / Down: move selection
- `v`: toggle rows / schema
- `Enter`: open row details
- `m`: open view/sort modal
- `M`: open filters modal
- `f`: search current table
- `F`: search all tables
- `r`: reload

## Notes

- Read-only by design
- BLOB values are shown as byte counts
- Some row-jump behavior depends on SQLite `rowid`

That triggers the GitHub release workflow and uploads platform archives for the installer scripts.
