# How Hangar works

Hangar does one thing on each scan: it reads what your AI tools already wrote to disk, turns it into project records, and stores those in a local SQLite file. It never writes to the tools' folders and never talks to the network.

## Sources

| Source | Where | What Hangar reads |
|---|---|---|
| Claude Code | `~/.claude/projects/*/*.jsonl` | `cwd`, `sessionId` and `timestamp` on every line |
| Codex | `~/.codex/sessions/YYYY/MM/DD/*.jsonl` | `session_meta` (session id, cwd), `turn_context` (cwd changes), `timestamp` on every line |
| Root folders | `roots` in `config.toml` | git repos up to 4 levels deep, including ones you never opened with an AI tool |

Lines that aren't valid JSON or don't have the fields above are skipped, so new log formats don't break a scan. Claude subagent transcripts (`<session>/subagents/`) are not read; their time already falls inside the parent session.

## Incremental import

Every log file is recorded with its size and modified time. On the next scan, unchanged files are skipped. A changed file has its old events replaced by a fresh import. If a log file is deleted, its events stay: Hangar never purges history.

## Project identity

A session's working folder is mapped to a project like this:

1. Walk up from the working folder to the nearest folder that contains `.git`. That folder is the project.
2. If there isn't one, the working folder itself is the project.

So a session in `my-app/src` counts toward `my-app`. Two clones of the same repo in different folders are two projects; the Location column tells them apart.

Paths are normalized first (resolved to their real on-disk spelling, trailing slashes dropped), so the logs and the root scan agree on one key per folder.

If a project folder disappears, the project stays and is flagged **missing**.

## Description and origin

- **Description:** the first prose paragraph of `README.md`, else `CLAUDE.md`, else `AGENTS.md`. Frontmatter, headings, badges, HTML, tables and code blocks are skipped, and inline markdown is reduced to plain text. The detail pane shows the whole file.
- **Origin:** the `url` of `[remote "origin"]` in the repo's git config (worktrees and submodules are followed). Credentials in the URL (`https://user:token@…`) are stripped before storing. Without a remote it's "local only", and without git it's "no git".

## Active time

Within one session, the gaps between consecutive log lines are added up. Gaps longer than 15 minutes count as idle and are dropped. A project's active time is the sum over all its sessions. The "7 days" column counts only gaps that end in the last 7 days.

Parallel sessions in the same project add up, so this measures agent time rather than wall-clock time.

## Storage

One SQLite file, `hangar.db`, in the data folder (see [configuration.md](configuration.md)):

| Table | Holds |
|---|---|
| `projects` | one row per project: automatic fields (name, origin, description, missing, host) and your fields (tags, status, notes) |
| `events` | one row per log line: file, project, tool, session, timestamp |
| `files` | size and modified time of every imported log file |

A scan only ever updates the automatic columns, so your tags, status and notes survive every rescan.
