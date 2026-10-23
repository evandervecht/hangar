# Find projects at risk

A project is **at risk** when its work exists in one place only: on this disk.

1. Tick **At risk only** in the top bar.
2. Read the **Git** column for each project:
   - `3 changed`: uncommitted changes. Commit them.
   - `2 unpushed`: commits on a local branch that no remote has, also on branches you don't have checked out. Push them.
   - `1 stashed`: work put aside with `git stash`. Apply it and commit, or drop it.
   - `local only`: a git repo without a remote. Add one (`git remote add origin …`) and push.
   - `no git`: not under version control at all. Run `git init`, or ignore it if it's scratch work.
3. Fix what matters, then click **Rescan** to update the list.

To get rid of noise in this view, select the project and use **Clean** → **Hide from Hangar**.

## When the Git column shows `?`

Git couldn't read the repo. Hover the `?` for a hint, or run `git status` in the folder. The usual cause on Windows is *dubious ownership*: the folder belongs to another user (for example `SYSTEM`). If the repo is yours and you trust it, run:

```sh
git config --global --add safe.directory C:/path/to/repo
```

Hangar doesn't skip this check itself, because it protects you from config in a repo you don't own running commands. A `?` project isn't counted as at risk, since Hangar can't tell.

## Before deleting a folder

**Clean** → **Move folder to Recycle Bin / Trash** shows a warning when the folder has uncommitted or unpushed work.
