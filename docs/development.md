# Development

## Layout

| File | What it does |
|---|---|
| `src/main.rs` | CLI entry point, data folder, `config.toml` |
| `src/scan.rs` | log parsing, root scan, SQLite schema and queries, plus the unit tests |
| `src/gui.rs` | the egui window |
| `SPEC.md` | v1 scope and what's deferred |

## Build and test

```sh
cargo run             # open the window against your real data
cargo run -- list     # CLI
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

CI (`.github/workflows/ci.yml`) runs fmt, clippy and the tests on Windows and macOS for every push to `main` and every pull request.

## Release

```sh
# bump version in Cargo.toml, commit, then:
git tag v0.1.0
git push origin v0.1.0
```

`.github/workflows/release.yml` creates the GitHub release and attaches:

- `hangar-<tag>-x86_64-pc-windows-msvc.zip`
- `hangar-<tag>-aarch64-apple-darwin.tar.gz` (Apple Silicon)
- `hangar-<tag>-x86_64-apple-darwin.tar.gz` (Intel Mac)

The binaries are unsigned. On macOS, clear the quarantine flag after unpacking:

```sh
xattr -d com.apple.quarantine ./hangar
```

Windows SmartScreen may warn on first launch; choose "More info" → "Run anyway".

## Adding another AI tool

1. Find where the tool stores per-session logs and which fields give the working folder, the session id and a timestamp.
2. Add the file discovery to `scan::run` next to the Claude and Codex ones.
3. Add a parser that turns one log line into `(cwd, session, timestamp)`, with a test using a real (anonymized) line.

Timestamps must be ISO 8601 in UTC (`2026-01-01T12:00:00.000Z`). SQLite's `julianday()` computes active time from them and returns NULL on anything it can't parse.
