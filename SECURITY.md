# Security

## Report a vulnerability

Use **Report a vulnerability** on the [Security tab](https://github.com/evandervecht/hangar/security/advisories/new). That stays private until a fix is out. Please don't open a public issue for it.

You'll get an answer within a week. Fixes go into the latest release only.

## What Hangar touches

- **Reads:** the session logs of Claude Code, Codex, Gemini CLI and Hermes Agent, and the git state and description files of your project folders. It treats them as untrusted: session ids are checked before they reach a shell, and CSV cells can't start a formula.
- **Writes:** its own SQLite file and `config.toml` in the data folder. It moves files to the Recycle Bin / Trash only when you confirm it in **Clean up**.
- **Network:** only when you click **Check for updates**, a request to the GitHub API for the latest release version.

## Releases

macOS builds are signed with a Developer ID and notarized by Apple. Windows builds aren't signed yet. Every release has a `SHA256SUMS` file; see [Check a download](docs/how-to/verify-download.md).

CI, and every release before it publishes, runs gitleaks over the whole git history for committed secrets and trivy over `Cargo.lock` for dependencies with a known HIGH or CRITICAL vulnerability that has a fix.
