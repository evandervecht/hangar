# Choose what Hangar scans

All of these can be done in **Settings** (**Hangar → Settings…** in the menu bar, or ⌘, on macOS and Ctrl+, on Windows), or by editing `config.toml` in the [data folder](../reference/configuration.md#data-folder) and restarting Hangar.

## Change the main project folder

1. Open **Hangar → Settings…**.
2. Under **Project folders**, type the folder path, for example `D:\Code`. A *folder not found* warning means the path doesn't exist.
3. Click **Save and rescan**.

Git repos up to 4 levels below the folder now appear, even if you never opened them with an AI tool.

## Scan more than one folder

In **Settings**, click **Add** under **Project folders** and type the next path. Every folder in the list is scanned.

## Use only the AI logs

In **Settings**, click **Remove** next to every project folder, then **Save and rescan**. Hangar then lists only projects from Claude Code and Codex sessions.

Projects that were found through a removed folder stay in the list, because Hangar never drops history. Use [Clean up](clean-up.md) to forget or hide them.

## Hide a single project

Select it, open **Clean** in the detail panel and click **Hide from Hangar**. Its path is added to **Hidden paths** in Settings. To bring it back, click **Remove** next to that path in Settings and **Save and rescan**.

## Hide a whole kind of folder

In **Settings**, click **Add** under **Hidden paths** and type a folder name, for example `SystemTemp` or `worktrees`. Then **Save and rescan**.

Names match whole folders only, so `Temp` doesn't hide `templates`. See [Configuration](../reference/configuration.md#excludes) for the exact rules.
