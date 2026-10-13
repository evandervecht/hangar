# How Hangar works

Hangar is a CMDB (configuration management database) for AI coding projects. A CMDB answers "what do we have, where is it, and what state is it in". Hangar asks those questions of your own disk, using what your AI tools already wrote down.

## Reading instead of watching

Claude Code and Codex already log every session to disk, including the working folder and a timestamp on every line. Hangar reads those logs on launch or on **Rescan**, so it needs no hooks, plugins or background process. The trade-off is that the list only updates when you ask it to; a background service is planned for v2.

Each log file is recorded with its size and modified time. Unchanged files are skipped on the next scan, so rescans are quick. A changed file is re-imported as a whole, because a session log can be rewritten, not just appended to.

Lines that aren't valid JSON or lack the fields Hangar needs are skipped. That keeps scans working when a tool changes its log format.

## What counts as a project

A session runs in a working folder, which is often a subfolder of a repo. Hangar walks up from that folder to the nearest one containing `.git` and calls that the project. Without git anywhere above it, the working folder itself is the project.

So a session in `my-app/src` counts toward `my-app`, and two clones of the same repo are two projects. That's intended: they live in different places and can be in different states, which is exactly what a CMDB tracks. The Location column tells them apart.

Paths are normalized to their real on-disk spelling first, so `c:\code\app\` from a log and `C:\Code\app` from the root scan become one project.

## Why history is never purged

AI tools clean up their own logs: Claude Code deletes transcripts after 30 days by default. If Hangar only showed what's in the logs right now, old projects would quietly lose their history. Hangar imports what it sees and keeps it, even after the log file or the project folder is gone. A missing folder is flagged *(missing)*, not removed. Forgetting a project is something only you do.

The same idea protects your own fields. A scan only updates the columns Hangar derives (name, origin, description, missing), never your tags, status or notes.

## Active time

Within a session, Hangar adds up the gaps between consecutive log lines. A gap longer than 15 minutes is counted as a break and left out. The result is an estimate of time the session was actually in use, not how long its window was open.

Parallel sessions in the same project each count, so three agents running for an hour count as three hours. That measures agent time, not your wall-clock time.

## Derived status

Most projects never get a status set by hand, so Hangar derives one: *active* when a session ran in the last 14 days, *idle* otherwise. It is shown in grey so you can tell it from a status you chose. Setting one by hand always overrides it, because you know things the logs don't, such as that a project is paused while you wait on someone.

## Why cleaning uses the Recycle Bin

Deleting log files or project folders can't be undone from inside Hangar, and a wrong click on a list of a hundred similar names is easy. So destructive cleaners move to the Recycle Bin / Trash, deleting a folder needs its name typed, and Hangar refuses to touch your home folder, scan roots and the AI tools' own data.

## Privacy

Everything stays on your machine. Hangar reads the logs and writes one local SQLite file. It doesn't connect to the network, and credentials embedded in git remote URLs are stripped before they are stored.
