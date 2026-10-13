# Configuration

## Data folder

| OS | Folder |
|---|---|
| Windows | `%APPDATA%\hangar` |
| macOS | `~/Library/Application Support/hangar` |
| Linux | `$XDG_DATA_HOME/hangar`, default `~/.local/share/hangar` |

| File | Holds |
|---|---|
| `config.toml` | settings, created with defaults on first run |
| `hangar.db` | all project data, including your tags, status and notes |

Deleting `hangar.db` (with Hangar closed) rebuilds it from the logs that still exist on the next scan. Manual fields are lost.

## config.toml

The **Settings** window edits `roots` and `excludes`; **Save and rescan** writes the file and scans again. The **Theme** menu writes `theme`. Changes made to the file by hand take effect on the next launch.

```toml
roots = ['C:\Users\you\Projects']
excludes = ["Temp", "scratchpad", "plugins/cache"]
theme = "hangar"
```

### roots

List of folders scanned for git repos.

| | |
|---|---|
| Type | list of paths |
| Default | `["~/Projects"]` (expanded to your home folder) |
| Depth | 4 levels below each root |
| Stops at | the first folder containing `.git` |
| Skips | `node_modules`, `target`, `.venv`, excluded paths |

### excludes

List of path patterns that hide projects.

| | |
|---|---|
| Type | list of strings |
| Default | `["Temp", "scratchpad", "plugins/cache"]` |
| Matching | whole path segments, case-insensitive; `/` or `\` separates segments in a pattern |
| Applied | when the list is shown and during the root scan; no rescan needed |

Examples: `Temp` hides `…\Local\Temp\…` but not `templates`. `plugins/cache` hides paths with `plugins` directly followed by `cache`. A full path hides that folder and everything under it.

### theme

| | |
|---|---|
| Type | string |
| Values | `""` or missing (System), `"dark"`, `"light"`, `"hangar"` |
| Default | System |
