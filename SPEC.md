# Hangar — v1 spec

A CMDB for your AI coding projects: where they live, what they are, how much time went into them.
Rust, one binary, Windows + macOS (Linux works too). Personal use, unsigned local builds.

## Interface
- `hangar` — egui window (scan on launch + Rescan button).
- `hangar scan` — import, print counts.
- `hangar list` — print the project table.

## Discovery
- Claude Code: `~/.claude/projects/*/*.jsonl` — `cwd`, `timestamp`, `sessionId` per line.
- Codex: `~/.codex/sessions/**/*.jsonl` — `session_meta` (id, cwd), `turn_context` (cwd), `timestamp` per line.
- Root scan: `config.toml` `roots` (default `~/Projects`), max 4 levels, stop at `.git`,
  skip `node_modules`, `target`, `.venv`. Only git repos count as projects here.
- Excludes (`config.toml`, whole path segments): `Temp`, `scratchpad`, `plugins/cache`.

## Project identity
Nearest parent folder containing `.git`, else the session cwd. Folder gone → flagged **missing**, never deleted.

## Record
- Automatic: path, name, description, origin (URL / "local only" / "no git"), tools used, session count,
  first seen, last active, active time (total, last 7 days; gaps > 15 min ignored), host.
- Manual: tags, status (active / paused / archived), notes.
- Description: first paragraph of `README.md`, else `CLAUDE.md`, else `AGENTS.md`. Full file shown in the detail pane.

## Detail actions
Open folder, open origin, copy path.

## Storage
SQLite (bundled) + `config.toml` in `%APPDATA%\hangar` / `~/Library/Application Support/hangar` / `~/.local/share/hangar`.
Only new/changed log files are re-imported. History is never purged.

## Install
`install.ps1` (Windows) / `install.sh` (macOS, Linux): build release, copy binary, create data folder.

## v0.2 additions
- Status derived from activity (active < 14 days, else idle) unless set by hand.
- Cleaners: forget missing projects, hide a project, trash old AI logs, trash a project folder (guarded).
- Themes: System, Dark, Light, Hangar. App icon on the Windows exe and in a macOS `Hangar.app`.
- Installers add Hangar to PATH. Docs follow Diátaxis.

## v0.4 additions (CMDB)
- Git health per repo (changed files, unpushed commits, last commit; `?` when git can't read it) and an at-risk filter.
- Clones: projects grouped by normalized origin URL.
- Moved projects: manual "Merge into" with path aliases so later log lines follow; missing projects keep last-known facts.
- Stack detection from marker files. Export: `hangar list --json | --csv`.

## v0.5 additions
- Window opens at once; scans run on a background thread (WAL, busy timeout); no console window from the Start Menu.
- Tray (Windows, macOS), close-to-tray, timed background scans, start at login (`--tray`), single instance via lock file.
- Gemini CLI and Hermes Agent sessions. SHA256SUMS in releases.

## v0.6 additions
- Activity stored per session per day (schema 3) instead of per log line; "7 days" = today and the 6 days before (UTC).
- Release files `hangar-<version>-windows-amd64.{exe,zip}` and `hangar-<version>-darwin-{arm64,amd64}.tar.gz`
  (ad-hoc signed `Hangar.app`).

## v0.7 additions
- Windows installer (`hangar-<version>-windows-amd64-setup.exe`, Inno Setup): per user, no admin, same folder as `install.ps1`,
  Start Menu shortcut, optional PATH entry and desktop shortcut. Uninstall removes PATH and login entries and asks
  before deleting the data folder.
- `hangar --version` / `--help`; stability promise for 1.0 in `docs/reference/stability.md`.

## Deferred
Next: Developer ID signing and notarization for macOS (waiting on the Apple Developer account).
Cursor sessions once its format is verified.
Maybe later (CMDB): cost per project across all AI tools (Claude Code, Codex, Cursor, Hermes, Gemini, …);
change history per project; owner/client field; several machines merged offline (export/import file).
Later: rendered markdown, resume session, more AI tools.
