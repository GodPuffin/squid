# Changelog

## v0.4.0

### Settings and session

- Settings screen (`,` from home or browse) with persistent preferences for color scheme, session restore, recents, SQL limits, live table search, row preview, and more.
- Per-database session state: filters, sorts, SQL editor, and optional cursor restore.

### Appearance

- Six color schemes: dark, light, monokai, solarized dark/light, and dracula.

### Row editing

- Insert new rows from browse (`a`) or the detail modal.
- Delete rows from browse (`d`) or the detail modal, with guards for read-only databases, missing rowid, and unsaved edits.

### Help and UX

- Context-sensitive help overlay (`?`) on home, browse, detail, search, SQL, and settings.
- Compact footer hints; modal footers no longer duplicate control text.

### CLI

- `--readonly` — open database read-only.
- `--scheme <name>` — set initial color scheme.
- `--no-session` — skip session restore and persist for this run.
