# Check a download

Every release has a `SHA256SUMS` file with the SHA-256 checksum of each file. Compare it with your download to be sure the file is the one GitHub Actions built.

## macOS and Linux

Put `SHA256SUMS` next to the download and run:

```sh
shasum -a 256 -c SHA256SUMS --ignore-missing
```

Your file should show `OK`.

## Windows (PowerShell)

```powershell
(Get-FileHash .\hangar-0.6.0-windows-amd64.exe -Algorithm SHA256).Hash.ToLower()
```

Compare the output with the line for that file in `SHA256SUMS`. They must match exactly.

## Check where it was built

`SHA256SUMS` sits in the same release as the files, so it only catches a damaged download. Each file also has a build provenance attestation that says which workflow built it. With the [GitHub CLI](https://cli.github.com):

```sh
gh attestation verify hangar-0.8.2-darwin-arm64.tar.gz --repo evandervecht/hangar
```

It prints `Verification succeeded` when the file was built by Hangar's release workflow.
