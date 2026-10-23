# Make a release

## Before tagging

1. Run the checks CI runs:
   ```sh
   cargo fmt --check
   cargo clippy --all-targets --locked -- -D warnings
   cargo test --locked
   ```
   To try the Windows installer too: `./packaging/windows-installer.ps1 target/release/hangar.exe <version> target/installer` (needs Inno Setup 6: `winget install JRSoftware.InnoSetup`).
2. Open the app with `cargo run --release` and switch through every theme in the **Theme** menu. Check that the table, the detail panel and the **Clean up** window are readable in each.
3. Bump `version` in `Cargo.toml`, run `cargo build` so `Cargo.lock` follows, and commit.

## Tag

```sh
git tag -a v0.2.0 -m "Hangar v0.2.0"
git push origin v0.2.0
```

A tag with a hyphen (`v0.8.1-rc.1`) makes a pre-release: use it to test a release, then delete it with `gh release delete <tag> --cleanup-tag`.

`.github/workflows/release.yml` creates the GitHub release with generated notes and attaches:

| File | Contains |
|---|---|
| `hangar-<version>-windows-amd64.zip` | `hangar.exe` (with icon), README, LICENSE (`<version>` is the tag without the `v`) |
| `hangar-<version>-darwin-arm64.tar.gz` | `Hangar.app` for Apple Silicon |
| `hangar-<version>-darwin-amd64.tar.gz` | `Hangar.app` for Intel Macs |
| `hangar-<version>-windows-amd64-setup.exe` | the Windows installer, built from `packaging/windows-installer.iss` with Inno Setup |
| `hangar-<version>-windows-amd64.exe` | the bare Windows executable |
| `SHA256SUMS` | checksums of all the files above, see [Check a download](verify-download.md) |

## Roll back a bad release

Fix forward when you can: a patch release (`v0.8.2` after a bad `v0.8.1`) reaches everyone who checks for updates.

When a release must disappear right away (it loses data, or a download is broken):

1. Turn it back into a draft, so it's no longer public and the previous release is **latest** again. **Check for updates** then points to that one.
   ```sh
   gh release edit v0.8.1 --draft
   ```
2. Say what happened in a GitHub issue, and which version to install instead (the previous release's files are unchanged).
3. Fix it, test with a pre-release tag (`v0.8.2-rc.1`), then release `v0.8.2`. Delete the draft and its tag once the fix is out:
   ```sh
   gh release delete v0.8.1 --cleanup-tag
   ```

Going back to an older Hangar usually keeps your data: database upgrades so far only add tables and columns, which an older version ignores. Downgrades aren't promised, though (see [Stability](../reference/stability.md)); before a release whose upgrade removes or changes something, say so in its notes.

Rehearse it now and then on a pre-release: tag `-rc.N`, turn it into a draft, delete it.

## macOS signing

From v0.8.1, `release.yml` signs `Hangar.app` with the Developer ID, has Apple notarize it and staples the ticket. It needs these repository secrets:

| Secret | Holds |
|---|---|
| `MACOS_CERT_P12` | the Developer ID Application certificate with its key, exported as `.p12`, base64 |
| `MACOS_CERT_PASSWORD` | the password of that `.p12` |
| `APPLE_TEAM_ID` | the 10-character Team ID |
| `NOTARY_KEY_P8` | an App Store Connect API key (`.p8`, Developer role), base64 |
| `NOTARY_KEY_ID`, `NOTARY_ISSUER_ID` | that key's ID and the issuer ID |

Notarization usually takes minutes; a new account's first submissions can wait hours. The **Notarize** step prints the submission id and waits up to 5 hours. On a Mac, `xcrun notarytool history` (with the same key) shows the queue.

## First launch

- **macOS, v0.8.1 and later:** signed and notarized; macOS only asks once whether to open an app downloaded from the internet. The first **Resume a session** asks to let Hangar control Terminal.
- **macOS, up to v0.8.0:** open it once; macOS blocks it. Go to **System Settings → Privacy & Security** and click **Open Anyway**, or run `xattr -dr com.apple.quarantine /Applications/Hangar.app`. If macOS says "Hangar is damaged and can't be opened" (releases up to v0.5.0 weren't signed at all), run that `xattr` command once.
- **macOS command line:** the CLI is inside the app: `/Applications/Hangar.app/Contents/MacOS/hangar list`. To type just `hangar`, run `ln -s /Applications/Hangar.app/Contents/MacOS/hangar /usr/local/bin/hangar` (or into any folder on your PATH).
- **Windows:** the binaries aren't signed yet, so SmartScreen may warn when you run the setup or the bare `.exe`. Choose **More info** → **Run anyway**.
