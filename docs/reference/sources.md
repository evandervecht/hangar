# Sources

Where Hangar finds sessions, and what it reads from each. Folders that don't exist are skipped.

| Tool | Where | What Hangar reads |
|---|---|---|
| Claude Code | `~/.claude/projects/*/*.jsonl` | `cwd`, `sessionId` and `timestamp` on every line; subagent transcripts are not read |
| Codex | `~/.codex/sessions/YYYY/MM/DD/*.jsonl` | `session_meta` (session id, cwd), `turn_context` (cwd changes), `timestamp` on every line |
| Gemini CLI | `~/.gemini/tmp/<project>/chats/*.jsonl` | `sessionId` and `startTime` on the first line, `timestamp` on messages; the folder comes from `<project>/.project_root` or `~/.gemini/projects.json`; subagent sessions are skipped |
| Hermes Agent | `$HERMES_HOME/state.db`, default `~/.hermes` (Windows: `%LOCALAPPDATA%\hermes`), plus `profiles/*/state.db` | SQLite, opened read-only: `sessions.id`, `sessions.cwd`, `messages.timestamp` (epoch seconds); sessions without a cwd are skipped |
| Root folders | `roots` in `config.toml` | git repos up to 4 levels deep |

Not supported yet: Cursor (its chat storage is internal and its timestamp fields aren't documented), GitHub Copilot CLI, OpenCode and Aider. See [Add another AI tool](../how-to/add-an-ai-tool.md).

The **Old AI session logs** cleaner only ever moves JSONL logs (Claude Code, Codex, Gemini CLI). It never touches Hermes' database.
