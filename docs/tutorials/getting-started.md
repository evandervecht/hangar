# Getting started

In this tutorial you install Hangar, look at the projects it finds on your machine, and record your first notes on one of them. It takes about ten minutes. You need to have used Claude Code or Codex at least once, so there are session logs to read.

## 1. Install

Download Hangar for your OS from [Releases](https://github.com/evandervecht/hangar/releases):

- **Windows:** download and run `hangar-<version>-windows-amd64-setup.exe`. It installs for your user only, without admin rights, adds a Start Menu shortcut and puts `hangar` on your PATH. If SmartScreen warns, choose **More info** → **Run anyway**.
- **macOS:** download `hangar-<version>-darwin-arm64.tar.gz` (Apple Silicon) or `hangar-<version>-darwin-amd64.tar.gz` (Intel), double-click it to unpack, and drag **Hangar** onto **Applications**. The first time you open it, macOS blocks it because it isn't signed: go to **System Settings → Privacy & Security** and click **Open Anyway**.

To build from source instead, clone the repo and run `./install.ps1` (Windows) or `./install.sh` (macOS). Those scripts also put `hangar` on your PATH.

## 2. Open the window

Start Hangar from the Start Menu, Applications, or a terminal:

```
hangar
```

On the first launch Hangar reads all your Claude Code and Codex logs, which takes a few seconds. You'll then see a table of projects, sorted by when you last worked on them.

## 3. Read the table

Look at the top row. That's the project you used most recently.

- **Location** is the folder the project lives in.
- **Status** shows *active* in grey: Hangar derived it, because you had a session in the last 14 days.
- **Active** is the time spent in AI sessions there; **7 days** is the part from today and the 6 days before.
- **Description** is the first paragraph of the project's README.

Type part of a project name into **Filter** and watch the table shrink.

## 4. Look at one project

Click a project name. A panel opens on the right with the full path, git origin, session count and the whole README.

Click **Open folder** to see it in Explorer or Finder. If the project has a GitHub origin, click **Open origin** to open it in your browser. Under **Resume a session**, click one of its last Claude Code or Codex sessions to continue it in a new terminal.

## 5. Record what you know

In the panel:

1. Type a tag in **Tags**, for example `client-a, rust`.
2. Set **Status** to *paused*.
3. Write a line in **Notes**, for example "Waiting for API access".
4. Click **Save**.

The Status cell in the table is now *paused* in normal text instead of grey: a status you set yourself always wins over the derived one. Click **Rescan**. Your tags, status and notes stay; a scan never overwrites them.

## 6. Try the command line

Open a new terminal:

```
hangar list
```

You get the same table as text, with full paths. Run `hangar scan` any time to import new sessions without opening the window.

## Where to go next

- Point Hangar at your own projects folder, or hide temp folders: [Choose what Hangar scans](../how-to/configure-sources.md)
- Remove projects you deleted: [Clean up projects and logs](../how-to/clean-up.md)
- Understand how active time is counted: [How Hangar works](../explanation/how-hangar-works.md)
