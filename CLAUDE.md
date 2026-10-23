# Hangar

A CMDB for AI coding projects: Rust (edition 2024), egui, bundled SQLite. Windows and macOS; Linux builds.
Scope is in SPEC.md, user docs in docs/ (Diátaxis), what Hangar won't do in README.md under **Not planned**.

## Checks

Run all three before saying a change works, and quote their result:

```sh
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
```

CI runs the same on Windows, macOS and Linux. Code behind `#[cfg(target_os = "macos")]` only compiles there:
keep logic in plain functions so it's tested everywhere (see `terminal_script` in src/gui.rs).

## Rules

- Never weaken or delete a test to make it pass. A bug fix gets a test that failed before the fix.
- Never read, print or commit signing keys or secrets (`*.p12`, `*.p8`, tokens). They live in GitHub secrets.
- Work on a branch and open a PR; `main` only changes through squash-merged PRs. Don't merge, tag or release
  unless asked.
- Session logs and READMEs are untrusted input: anything from them that reaches a shell, a CSV cell or a URL is
  checked first (see `resume`, `csv`).
- Keep the stability promise (docs/reference/stability.md): new CSV columns go last, database migrations only
  add and run twice safely, config keys keep their defaults.
- Comment only the why. Update the docs a change touches, and add a line to SPEC.md under the next version.

## Release

docs/how-to/release.md. Bump `version` in Cargo.toml in a PR (`main` is protected: PRs with green CI only),
then tag the merged commit on main with `v<version>`. Test release
changes first with a hyphen tag (`v0.8.2-rc.1`), which makes a pre-release.
