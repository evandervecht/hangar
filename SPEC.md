# Hangar — v1 spec

A CMDB for your AI coding projects: where they live, what they are, how much time went into them.
Rust, one binary, Windows + macOS (Linux works too). Personal use; macOS releases signed and notarized, Windows builds unsigned.

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
Nearest parent folder containing `.git`, else the session cwd. Subfolders a session moves into count toward the folder it started in, except a git repo inside a non-repo start folder, which keeps its own time (schema 4). Folder gone → flagged **missing**, never deleted.

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

## v0.8 additions
- Resume one of a project's last 5 Claude Code / Codex sessions from the detail pane: `claude --resume <id>` /
  `codex resume <id>` in a new terminal, in the folder the session started in. Only ids of letters, digits and `-`.

## v0.8.1 additions
- macOS releases signed with the Developer ID (hardened runtime, apple-events entitlement for Resume),
  notarized and stapled in `release.yml`. A tag with a hyphen (`v0.8.1-rc.1`) makes a pre-release.

## v0.8.2 additions
- Git: unpushed counts every local branch (`rev-list HEAD --branches --not --remotes`), new `git_stashed`
  (schema 5; at risk when > 0; exported last in CSV). Column headers explain themselves on hover (ℹ).
- REVIEW.md at the repo root: what the automated review lanes treat as blocker/major, minor and skip.
- The tray menu starts with the running version (`Hangar 0.8.x`, greyed out).
- CI `security` job: gitleaks over the full history, trivy (vuln, misconfig; HIGH/CRITICAL with a fix) over the repo.
- Menu bar in the window: **Hangar** (Settings… Ctrl/⌘+,, Check for updates, Clean up…, Quit) and **View → Theme**;
  the top bar keeps Filter, At risk only and Rescan. macOS: Hangar's own app menu (About, Settings… ⌘,, Hide,
  Quit ⌘Q, which really quits in background mode) and a Window menu.
- Hardening (#21): the delete guard also refuses `~/.ssh`, AppData, `~/Library`, Hangar's data folder and Documents
  itself, and runs again at trash time; **Move … files** re-checks each log's age; Unix data folder `0700`, `hangar.db`
  and `config.toml` `0600`; `config.toml.tmp` is created fresh (never through a symlink); scratchpads skip symlinks and
  other users' folders; the project walk stops at home and at folders another account owns; the `.desktop` `Exec` is escaped.

## Deferred
Next: a signed Windows installer.
Cursor sessions once its format is verified.
Maybe later (CMDB): change history per project; owner/client field; several machines merged offline (export/import file).
Later: rendered markdown, more AI tools.
Not planned: cost or token counts, conversation search, session memory, handing a session to another
tool or model, launching agents (Hangar isn't a launcher; AgentsView covers sessions and cost).
