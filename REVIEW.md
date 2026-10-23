# Review rules

What the review lanes hold a PR in this repo to. CLAUDE.md has the house rules; this file says what is worth raising.

## Important (blocker or major)
- A test deleted, weakened or changed to pass instead of the code being fixed; a bug fix without a test that failed before it.
- A break of the stability promise (`docs/reference/stability.md`): a CSV column not added last or renamed, a `--json` field renamed or retyped, a CLI flag or exit code changed, a `config.toml` key whose default changed.
- A database migration that drops or rewrites data, or that is not safe to run twice.
- Session logs, READMEs or other project text reaching a shell, a CSV cell or a URL unchecked (see `resume`, `csv`); a session id outside letters, digits and `-`.
- A signing key, secret or token (`*.p12`, `*.p8`) read, printed or committed, or a release workflow change that exposes one.
- Logic only behind `#[cfg(target_os = "macos")]` that CI cannot test on every OS (keep it in plain functions, like `terminal_script`).
- A change that fails `cargo fmt --check`, `cargo clippy --all-targets --locked -- -D warnings` or `cargo test --locked`, or a `Cargo.lock` that is not committed with a dependency change.
- A release change (`Cargo.toml` version, tags, `release.yml`) in a PR that is not about a release.
- A user-visible change without its docs under `docs/` and a line in `SPEC.md` under the next version.

## Minor
- A new helper or crate that duplicates what the code or an installed crate already does.
- A comment, README line or doc page that still describes the old behaviour, or a comment that says what instead of why.
- A PR body missing a `## Merge Danger` call (`**Door:**`, `**Blast Radius:**`) or `## Issues`.

## Skip
- Style and formatting that rustfmt and clippy already own.
- Asking for abstractions, options or config nobody asked for.
- Anything under README **Not planned** (cost or token counts, conversation search, launching agents).
- More than three nits per lane.
