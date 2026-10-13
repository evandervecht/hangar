# Configuration

## Data folder

Hangar keeps two files in one folder per user:

| OS | Folder |
|---|---|
| Windows | `%APPDATA%\hangar` |
| macOS | `~/Library/Application Support/hangar` |
| Linux | `$XDG_DATA_HOME/hangar`, default `~/.local/share/hangar` |

- `config.toml`: your settings, created with defaults on first run.
- `hangar.db`: everything Hangar knows, including your tags, status and notes. Back this file up if those matter to you.

To start over, close Hangar and delete `hangar.db`. The next scan rebuilds it from the logs that still exist. Your manual fields are lost.

## config.toml

```toml
roots = ['C:\Users\you\Projects']
excludes = ["Temp", "scratchpad", "plugins/cache"]
```

### roots

Folders to scan for git repos, up to 4 levels deep. The scan stops descending once it finds a `.git`, and it skips `node_modules`, `target` and `.venv`. The default is `~/Projects`. Use an empty list (`roots = []`) to rely on the AI logs only.

On Windows, use single quotes so backslashes aren't treated as escapes: `'D:\Code'`.

### excludes

Path segments that hide a project, case-insensitive. A pattern matches whole folder names only:

- `Temp` hides `…\AppData\Local\Temp\…` but not `templates`.
- `plugins/cache` hides any path containing `plugins` directly followed by `cache`.

Excludes are applied when the list is shown, so editing them takes effect on the next launch without a rescan. Common additions: `SystemTemp` (Windows), `worktrees`, `node_modules`.
