# How Hangar works

Hangar is a CMDB (configuration management database) for AI coding projects. A CMDB answers "what do we have, where is it, and what state is it in". Hangar asks those questions of your own disk, using what your AI tools already wrote down.

## Reading instead of watching

Claude Code and Codex already log every session to disk, including the working folder and a timestamp on every line. Hangar reads those logs on launch or on **Rescan**, so it needs no hooks, plugins or background process. The trade-off is that the list only updates when you ask it to; a background service is planned for v2.

Each log file is recorded with its size and modified time. Unchanged files are skipped on the next scan, so rescans are quick. A changed file is re-imported as a whole, because a session log can be rewritten, not just appended to.

Lines that aren't valid JSON or lack the fields Hangar needs are skipped. That keeps scans working when a tool changes its log format.

## What counts as a project

A session runs in a working folder, which is often a subfolder of a repo. Hangar walks up from that folder to the nearest one containing `.git` and calls that the project. Without git anywhere above it, the working folder itself is the project.

The agent can change folders during a session, for example to clone a repo into a scratch folder and work in it. Hangar then looks at the folder the session started in:

- A plain subfolder (`src`, `scripts`) counts toward the start folder, so it doesn't become a project of its own.
- A git repo inside a start folder that isn't one (a clone in a scratch or workspace folder) keeps its own row and time. A repo nested in that repo (a submodule, vendored code) counts toward it.
- If the start folder lies in a repo, everything inside it counts toward that repo.
- A folder outside the start folder is its own project again, as above.

So a session in `my-app/src` counts toward `my-app`, and two clones of the same repo are two projects. That's intended: they live in different places and can be in different states, which is exactly what a CMDB tracks. The Location column tells them apart.

Paths are normalized to their real on-disk spelling first, so `c:\code\app\` from a log and `C:\Code\app` from the root scan become one project.

## Why history is never purged

AI tools clean up their own logs: Claude Code deletes transcripts after 30 days by default. If Hangar only showed what's in the logs right now, old projects would quietly lose their history. Hangar imports what it sees and keeps it, even after the log file or the project folder is gone. A missing folder is flagged *(missing)*, not removed. Forgetting a project is something only you do.

The same idea protects your own fields. A scan only updates the columns Hangar derives (name, origin, description, missing), never your tags, status or notes.

## Active time

Within a session, Hangar adds up the gaps between consecutive log lines. A gap longer than 15 minutes is counted as a break and left out. The result is an estimate of time the session was actually in use, not how long its window was open. Hangar stores the outcome per session per day, not every log line, so the database stays small however long you use it.

Parallel sessions in the same project each count, so three agents running for an hour count as three hours. That measures agent time, not your wall-clock time.

## Derived status

Most projects never get a status set by hand, so Hangar derives one: *active* when a session ran in the last 14 days, *idle* otherwise. It is shown in grey so you can tell it from a status you chose. Setting one by hand always overrides it, because you know things the logs don't, such as that a project is paused while you wait on someone.

## Why cleaning uses the Recycle Bin

Deleting log files or project folders can't be undone from inside Hangar, and a wrong click on a list of a hundred similar names is easy. So destructive cleaners move to the Recycle Bin / Trash, deleting a folder needs its name typed, and Hangar refuses to touch your home folder, scan roots and the AI tools' own data.

## Health: is the work safe?

A CMDB doesn't just list what exists; it flags what's out of order. For code, the question that matters is whether the work exists anywhere else. Hangar asks git on every scan: are there uncommitted changes, commits that are on no remote, or no remote at all? Any of those, or no git at all, makes a project **at risk**: losing this disk means losing that work.

Hangar runs git with `--no-optional-locks`, so it never takes the index lock in a repo where an agent is working. When git refuses a repo, usually because the folder belongs to another user, Hangar shows `?` instead of guessing. It deliberately doesn't override git's ownership check, since that check stops a foreign repo's config from running commands on your machine.

## Clones are relationships

The same repo checked out in several folders is one asset with several instances. Hangar keeps each checkout as its own project, because each can be in a different state (one clean, one with unpushed work), and links them through their origin. The **Clones** column and the detail panel's clone list show that relationship. URLs are compared after normalizing, so SSH and HTTPS checkouts of one repo count as clones.

## Why merging a moved project is manual

To Hangar, a moved folder looks like a missing project plus a new project. To git, a clone looks the same: same origin, same history. So "missing project with the same origin as an existing one" can mean *moved*, or *one clone deleted while another remains*. Merging automatically would get the second case wrong and blend two instances' histories. Hangar therefore only suggests candidates and lets you decide. Once you merge, it remembers the move, so later log lines from the old path land on the new project.

The same reasoning is why a missing project keeps its last-known origin, description and git facts instead of being reset: those are what identify it later.

## Why other tools are added slowly

Each AI tool stores sessions its own way, and most of those formats are internal. Gemini CLI and Hermes Agent are supported because their storage is open source or documented. Cursor is not yet: its chats live in a large internal SQLite database whose timestamp fields aren't documented, and a wrong guess would show wrong hours. Hermes' database is a live file of another program, so Hangar opens it read-only and re-reads it when it changes. A very large store makes those rescans slower.

## Running in the background

Hangar's "service" is the app itself, hidden in the tray, not an operating-system service. The session logs live in your home folder and belong to your login, so a per-user process that starts at login is the right owner. Only one copy runs per user: the window holds a lock file in the data folder, and a second start leaves a small request file there and exits, which makes the running copy show its window. No network sockets are involved.

Scans run on a background thread with their own database connection, so the window stays responsive. Edits wait until a scan finishes, because SQLite allows one writer at a time.

## Privacy

Everything stays on your machine. Hangar reads the logs and writes one local SQLite file. It doesn't connect to the network on its own: only when you choose **Check for updates** does it ask GitHub for the latest release version, sending nothing about your projects. Credentials embedded in git remote URLs are stripped before they are stored.
