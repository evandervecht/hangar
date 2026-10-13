# Choose what Hangar scans

## Add a folder of repos you never opened with an AI tool

1. Open `config.toml` in the [data folder](../reference/configuration.md#data-folder).
2. Add the folder to `roots`. On Windows, use single quotes so backslashes work:
   ```toml
   roots = ['C:\Users\you\Projects', 'D:\Code']
   ```
3. Click **Rescan**, or run `hangar scan`.

Git repos up to 4 levels below each root now appear, even without any AI sessions.

## Use only the AI logs

Set an empty list:

```toml
roots = []
```

## Hide a single project

Select it, open **Clean** in the detail panel and click **Hide from Hangar**. Hangar adds its path to `excludes`. To bring it back, remove that line from `config.toml` and restart Hangar.

## Hide a whole kind of folder

Add a folder name to `excludes`, for example worktrees or Windows' system temp folder:

```toml
excludes = ["Temp", "scratchpad", "plugins/cache", "SystemTemp", "worktrees"]
```

Restart Hangar. Patterns match whole folder names, so `Temp` doesn't hide `templates`. See [Configuration](../reference/configuration.md#excludes) for the exact rules.
