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

## Deferred
v2: tray icon + background service. Later: rendered markdown, resume session, more AI tools,
multi-machine merge, signing/notarization, CI.
