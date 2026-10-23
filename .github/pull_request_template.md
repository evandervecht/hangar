## Summary

<!-- The smallest visual that makes the change clear: a before/after table, a code block, a list. -->

## Evidence

<!-- Before and after: what ran and its result. -->

- [ ] `cargo fmt --check`, `cargo clippy --all-targets --locked -- -D warnings` and `cargo test --locked` pass
- [ ] A bug fix has a test that failed before the fix
- [ ] Tried in the app, or said here why not

## Merge Danger

**Door:** two-way <!-- one-way: hard to undo (a migration that drops data, a renamed CSV column) -->
**Blast Radius:** small <!-- small / medium / large, and what it touches -->

- [ ] Stability promise kept (docs/reference/stability.md): CLI, `--json`/`--csv`, `config.toml`, database
- [ ] Docs and SPEC.md updated where the change shows

## Issues

<!-- Open issues this touches (gh issue list --state open), or "none". -->
