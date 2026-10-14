# Stability

From v1.0.0, Hangar follows [semantic versioning](https://semver.org/). These are the parts you can build on. Breaking one of them means a new major version.

| Part | Stable | May change in a minor release |
|---|---|---|
| [CLI](cli.md) | commands, flags, exit codes (`0` ok, `1` error, `2` usage) | new commands and flags |
| `hangar list --json` | field names, their types and meaning ([export fields](project-fields.md#export-fields)) | new fields |
| `hangar list --csv` | column names and their order | new columns, added at the end |
| [config.toml](configuration.md#configtoml) | keys, values and defaults | new keys; a missing key always takes its default |
| [Data folder](configuration.md#data-folder) | location and file names | new files |
| `hangar.db` | a newer Hangar opens and upgrades any older database, keeping your tags, status and notes | new tables and columns; reading it with your own tools is fine, writing to it is not supported |

Not covered:

- The text table of plain `hangar list`. It's meant for people; scripts use `--json` or `--csv`.
- The window: layout, wording, themes and colours.
- Downgrades: an older Hangar may not read a database a newer one upgraded (see [Database](project-fields.md#database)).
- The session logs of Claude Code, Codex, Gemini CLI and Hermes Agent. They belong to those tools; when one changes its format, Hangar follows in a minor release.
