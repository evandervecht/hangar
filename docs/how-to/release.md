# Make a release

## Before tagging

1. Run the checks CI runs:
   ```sh
   cargo fmt --check
   cargo clippy --all-targets -- -D warnings
   cargo test
   ```
2. Open the app with `cargo run --release` and switch through every theme in the **Theme** menu. Check that the table, the detail panel and the **Clean up** window are readable in each.
3. Bump `version` in `Cargo.toml`, run `cargo build` so `Cargo.lock` follows, and commit.

## Tag

```sh
git tag -a v0.2.0 -m "Hangar v0.2.0"
git push origin v0.2.0
```

`.github/workflows/release.yml` creates the GitHub release with generated notes and attaches:

| File | Contains |
|---|---|
| `hangar-<tag>-x86_64-pc-windows-msvc.zip` | `hangar.exe` (with icon), README, LICENSE |
| `hangar-<tag>-aarch64-apple-darwin.tar.gz` | `Hangar.app`, the bare `hangar` binary, README, LICENSE (Apple Silicon) |
| `hangar-<tag>-x86_64-apple-darwin.tar.gz` | the same for Intel Macs |
| `hangar-<tag>-x86_64-pc-windows-msvc.exe` | the bare Windows executable |
| `hangar-<tag>-aarch64-apple-darwin` | the bare macOS binary, Apple Silicon (CLI use) |
| `hangar-<tag>-x86_64-apple-darwin` | the bare macOS binary, Intel |

## Unsigned binaries

The binaries are not signed yet.

- **macOS:** right-click `Hangar.app` and choose **Open** the first time. For the bare binary, run `xattr -d com.apple.quarantine ./hangar`.
- **Windows:** SmartScreen may warn on first launch. Choose **More info** → **Run anyway**.
