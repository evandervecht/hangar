# Project fields

## Columns

| Column | Source | Meaning |
|---|---|---|
| Project | automatic | folder name; `(missing)` when the folder no longer exists |
| Location | automatic | the folder the project lives in |
| Status | derived or manual | see [Status](#status) |
| Git | automatic | see [Git](#git) |
| Stack | automatic | see [Stack](#stack) |
| Clones | automatic | how many other visible projects have the same origin repo; empty when none |
| Tags | manual | free text |
| Tools | automatic | AI tools with sessions here: `claude`, `codex`, `gemini`, `hermes` |
| Sessions | automatic | number of distinct sessions |
| Active | automatic | total active time, in hours |
| 7 days | automatic | active time today and the 6 days before (UTC calendar days) |
| Last active (UTC) | automatic | time of the most recent log line |
| Description | automatic | first paragraph of `README.md`, else `CLAUDE.md`, else `AGENTS.md` |

## Scratchpads

A project with Claude Code scratchpads gets a **▸** before its name. Click it to fold out one sub-row per scratchpad, newest first:

| Column | Meaning |
|---|---|
| Project | `↳ scratchpad <first 8 characters of the session id>`; click to open the folder |
| Location | the session folder the scratchpad is in |
| Status | always *temp* |
| Last active (UTC) | newest change among the files and folders directly inside it |
| Description | how many files and folders are directly inside it |

Hangar looks in `<temp>/claude/<project>/<session id>/scratchpad` (on Windows `%TEMP%\claude\...`). The session id links each scratchpad to the project that session ran in. Empty scratchpads, and those of sessions Hangar never imported, are left out. Scratchpads are only shown, never stored: when Claude Code or the OS clears the temp folder, they disappear from Hangar too.

The detail panel also shows the full path, origin, first-seen date, last commit date, the other clones, notes and the full description file.

## Status

| Value | Set by | Meaning |
|---|---|---|
| active (grey) | Hangar | a session ran in the last 14 days |
| idle (grey) | Hangar | no session in the last 14 days, or none at all |
| active / paused / archived | you | your choice; always wins over the derived value |

Choose **auto** in the detail panel to go back to the derived value.

## Git

Read on every scan with `git status`, `git rev-list` and `git log`, for projects that exist and aren't hidden.

| Value | Meaning |
|---|---|
| `clean` | nothing uncommitted, every commit is on a remote |
| `N changed` | N entries in `git status`: changed or untracked files (an untracked folder counts once) |
| `N unpushed` | N commits that are on no remote branch |
| `local only` | a repo without an `origin` remote |
| `no git` | not a git repo |
| `?` | git couldn't read the repo, for example *dubious ownership* or git not installed |
| empty | the folder is gone; the last-known values are kept for merging |

### At risk

A project is **at risk** when it exists and its Git value is `no git`, `local only`, or has changed files or unpushed commits. `?` is not at risk, because Hangar can't tell. At-risk values are shown in the warning colour, and **At risk only** filters on them.

## Stack

Detected from files in the project folder itself (not subfolders):

| File | Stack |
|---|---|
| `Cargo.toml` | rust |
| `package.json` | node |
| `tsconfig.json` | typescript |
| `pyproject.toml`, `requirements.txt` | python |
| `go.mod` | go |
| `pom.xml`, `build.gradle` | java |
| `Gemfile` | ruby |
| `composer.json` | php |
| `Dockerfile` | docker |
| `pubspec.yaml` | dart |
| `*.tf` | terraform |
| `*.csproj`, `*.sln` | dotnet |

## Origin

| Value | Meaning |
|---|---|
| a URL | the `origin` remote, with any credentials removed |
| `local only` | a git repo without an `origin` remote |
| `no git` | not a git repo |

Clones are matched on the origin regardless of URL style: `git@host:a/b.git`, `https://host/a/b` and `ssh://user@host:22/a/b.git` are the same repo.

## Export fields

`hangar list --json` and `--csv` contain:

`path`, `name`, `status` (CSV: the shown status; JSON: `status` is your manual value and `shown_status` the displayed one), `tags`, `notes`, `origin`, `clones`, `stack`, `git_changed`, `git_unpushed`, `git_last_commit`, `at_risk`, `missing`, `tools`, `sessions`, active time (JSON: `active_secs`, `week_secs`; CSV: `active_hours`, `week_hours`), `first_seen`, `last_active`, `description`.

Unknown git values are `null` in JSON and empty in CSV.

## Database

`hangar.db` is SQLite, schema version 4 (`PRAGMA user_version`).

| Table | Columns |
|---|---|
| `projects` | `path` (key), `name`, `origin`, `description`, `missing`, `host`, `first_seen`, `tags`, `status`, `notes`, `git_changed`, `git_unpushed`, `git_last_commit`, `stack` |
| `activity` | `file`, `project`, `tool`, `session`, `day`, `first`, `last`, `secs` (one row per session per UTC day; `secs` is active time with idle gaps left out) |
| `files` | `path` (key), `mtime`, `size` (one row per imported log file) |
| `moves` | `from_path` (key), `to_path` (one row per merged project) |

Schema 4 changes no tables: it makes the next scan import every log that still has history again, so sessions follow the start-folder rule in [How Hangar works](../explanation/how-hangar-works.md). Leftover subfolder projects without sessions or anything you typed are removed. Logs of projects you forgot stay skipped.

Schema 3 replaced the v0.5 `events` table (one row per log line) with `activity`. Opening a database with v0.6 converts it once and shrinks the file; an older Hangar opened afterwards shows no sessions until you upgrade again.
