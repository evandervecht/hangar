# Add another AI tool

1. Find where the tool stores its per-session logs, and which fields hold the working folder, a session id and a timestamp.
2. In `src/scan.rs`, add the files to `log_files()` with a new tool name, next to `claude` and `codex`.
3. Write a parser that turns one log line into `(cwd, session, timestamp)` and call it from `import_file()`. If the tool only writes the working folder once per session, keep state the way the `Codex` parser does.
4. Add a unit test that feeds it real, anonymized log lines.
5. Run `cargo run -- scan` and then `cargo run -- list`, and check the new tool appears in the **Tools** column.

Timestamps must be ISO 8601 in UTC (`2026-01-01T12:00:00.000Z`). SQLite's `julianday()` computes active time from them, and it returns NULL for anything it can't parse, which silently zeroes the time.

Source layout, for orientation:

| File | What it does |
|---|---|
| `src/main.rs` | CLI entry point, data folder, `config.toml` |
| `src/scan.rs` | log parsing, root scan, SQLite schema and queries, cleaners, unit tests |
| `src/gui.rs` | the egui window and themes |
| `build.rs` | embeds the Windows icon |
| `packaging/macos-app.sh` | builds `Hangar.app` |
