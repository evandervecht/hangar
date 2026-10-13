# Merge a moved project

When you move or rename a project folder, Hangar sees two things: the old path is *(missing)*, and the new path is a new project without history. Merge them to keep one record.

1. Click **Rescan** so Hangar sees the new location.
2. Select the old project, marked *(missing)*.
3. In the detail panel, under **Folder is gone**, click **Merge into …** next to the new location.

Hangar suggests projects with the same git origin. Projects without git are suggested when they have the same folder name.

What merging does:

- Moves all sessions and active time to the new project.
- Copies tags, status and notes from the old project where the new one has none.
- Keeps the earlier first-seen date.
- Remembers the move, so log lines from the old path (for example an old session you resume) land on the new project in later scans.

Merging can't be undone from the app. If no suggestion appears, the new location hasn't been scanned yet: add its folder in **Settings** or run a session there, then **Rescan**.
