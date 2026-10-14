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
| `hangar-<version>-windows-amd64.zip` | `hangar.exe` (with icon), README, LICENSE (`<version>` is the tag without the `v`) |
| `hangar-<version>-darwin-arm64.tar.gz` | `Hangar.app` for Apple Silicon |
| `hangar-<version>-darwin-amd64.tar.gz` | `Hangar.app` for Intel Macs |
| `hangar-<version>-windows-amd64.exe` | the bare Windows executable |
| `SHA256SUMS` | checksums of all the files above, see [Check a download](verify-download.md) |

## Unsigned binaries

The binaries are not signed yet. Signing is in progress: macOS builds will be signed and notarized once Apple approves the developer account.

- **macOS:** after dragging Hangar to Applications, open it once; macOS blocks it. Go to **System Settings → Privacy & Security** and click **Open Anyway**. Or run `xattr -dr com.apple.quarantine /Applications/Hangar.app`.
- **macOS says "Hangar is damaged and can't be opened":** that's a downloaded app without a valid signature (releases up to v0.5.0 weren't signed at all). Run `xattr -dr com.apple.quarantine /Applications/Hangar.app` once.
- **macOS command line:** the CLI is inside the app: `/Applications/Hangar.app/Contents/MacOS/hangar list`. To type just `hangar`, run `ln -s /Applications/Hangar.app/Contents/MacOS/hangar /usr/local/bin/hangar` (or into any folder on your PATH).
- **Windows:** SmartScreen may warn on first launch. Choose **More info** → **Run anyway**.
