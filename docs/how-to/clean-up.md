# Clean up projects and logs

Hangar has four cleaners. The first two only change Hangar's own database. The last two touch files on disk and move them to the **Recycle Bin / Trash**, so you can restore them.

## Forget projects whose folder is gone

1. Choose **Hangar → Clean up…** in the menu bar.
2. Under **Missing projects**, click **Forget all missing**. This forgets the missing projects in the list; hidden ones stay in the database.

To forget one: select a project marked *(missing)*, open **Clean** in the detail panel and click **Forget**.

Forgetting removes the project and its session history from Hangar. If the folder comes back, or a new session runs there, the project reappears on the next scan.

## Hide noise

Select the project, open **Clean** and click **Hide from Hangar**. See [Choose what Hangar scans](configure-sources.md#hide-a-single-project).

## Free space taken by old AI session logs

1. Choose **Hangar → Clean up…** in the menu bar.
2. Under **Old AI session logs**, set the age in days (at least 7) and click **Find**.
3. Check the file count and size, then click **Move … files to the Recycle Bin / Trash**.

This moves Claude Code transcripts (with their subagent and tool-result folders) and Codex rollouts. Hangar keeps the history it already imported, so active time and session counts don't change. Claude Code and Codex can no longer resume those sessions.

## Delete a project folder

1. Select the project and open **Clean** in the detail panel.
2. Type the project's name exactly as shown.
3. Click **Move folder to Recycle Bin / Trash**.

The project stays in Hangar, marked *(missing)*, until you forget it.

Hangar refuses to delete your home folder or anything above it, a configured scan root (or anything above one), and the AI tools' own data folders (`~/.claude`, `~/.codex`, `~/.gemini` and the Hermes Agent folder). Paths are compared after resolving their real spelling, so a root typed in another case still counts.

## Uninstall Hangar (Windows)

Open **Settings → Apps → Installed apps**, find **Hangar** and choose **Uninstall**. It closes Hangar if it runs, and removes the program, its shortcuts, its PATH entry and its start-at-login entry.

It then asks whether to delete your Hangar data (`%APPDATA%\hangar`: projects, tags, notes and settings). The default is **No**, so a later install picks up where you left off. Your projects and AI session logs are never touched.
