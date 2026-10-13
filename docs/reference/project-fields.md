# Project fields

## Columns

| Column | Source | Meaning |
|---|---|---|
| Project | automatic | folder name; `(missing)` when the folder no longer exists |
| Location | automatic | the folder the project lives in |
| Status | derived or manual | see [Status](#status) |
| Tags | manual | free text |
| Tools | automatic | AI tools with sessions here: `claude`, `codex` |
| Sessions | automatic | number of distinct sessions |
| Active | automatic | total active time, in hours |
| 7 days | automatic | active time in the last 7 days |
| Last active (UTC) | automatic | time of the most recent log line |
| Description | automatic | first paragraph of `README.md`, else `CLAUDE.md`, else `AGENTS.md` |

The detail panel also shows the full path, the git origin, the first-seen date, notes, and the full description file.

## Status

| Value | Set by | Meaning |
|---|---|---|
| active (grey) | Hangar | a session ran in the last 14 days |
| idle (grey) | Hangar | no session in the last 14 days, or none at all |
| active / paused / archived | you | your choice; always wins over the derived value |

Choose **auto** in the detail panel to go back to the derived value.

## Origin

| Value | Meaning |
|---|---|
| a URL | the `origin` remote, with any credentials removed |
| `local only` | a git repo without an `origin` remote |
| `no git` | not a git repo |

## Database

`hangar.db` is SQLite.

| Table | Columns |
|---|---|
| `projects` | `path` (key), `name`, `origin`, `description`, `missing`, `host`, `first_seen`, `tags`, `status`, `notes` |
| `events` | `file`, `project`, `tool`, `session`, `ts` (one row per log line) |
| `files` | `path` (key), `mtime`, `size` (one row per imported log file) |

`PRAGMA user_version` holds the schema version (currently 1).
