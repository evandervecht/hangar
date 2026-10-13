<p align="center"><img src="assets/hangar.png" width="160" alt="Hangar logo"></p>

# Hangar

[![CI](https://github.com/evandervecht/hangar/actions/workflows/ci.yml/badge.svg)](https://github.com/evandervecht/hangar/actions/workflows/ci.yml)

A CMDB for your AI coding projects. Hangar tracks where your Claude Code and Codex projects live, what they are, their git origin and the time you've spent in each. It's built in Rust for Windows and macOS.

## What it does

- **Finds your projects** from the session logs Claude Code (`~/.claude/projects`) and Codex (`~/.codex/sessions`) leave behind, plus git repos under your work folders.
- **Shows what each one is.** The description comes from the first paragraph of `README.md`, `CLAUDE.md` or `AGENTS.md`.
- **Shows where it lives and where it syncs to:** the path and the git origin, or "local only" / "no git".
- **Shows the time spent:** sessions, active time (total and the last 7 days, with idle gaps over 15 minutes left out) and when the project was last active.
- **Shows the state:** *active* or *idle* from recent activity, unless you set active / paused / archived yourself. You can also add tags and notes.
- **Never forgets:** a project whose folder is gone stays in the list, flagged *missing*.
- **Cleans up:** forget missing projects, hide noise, and move old AI session logs or whole project folders to the Recycle Bin / Trash.
- **Themes:** System, Dark, Light, or Hangar (navy with the logo's coral and cyan accents).

Everything stays on your machine. Hangar only reads the logs and writes its own local SQLite file.

## Install

**Download:** grab the zip (Windows) or the tar.gz with `Hangar.app` (macOS) from [Releases](https://github.com/evandervecht/hangar/releases). The binaries are unsigned; see [Make a release](docs/how-to/release.md#unsigned-binaries) for the first-launch step.

**From source:** you need a [Rust toolchain](https://rustup.rs).

```sh
git clone https://github.com/evandervecht/hangar.git
cd hangar
./install.ps1   # Windows: %LOCALAPPDATA%\Programs\hangar, Start Menu shortcut, adds to PATH
./install.sh    # macOS / Linux: ~/.local/bin (added to PATH), plus ~/Applications/Hangar.app on macOS
```

## Use

```
hangar          # open the window (scans on launch, Rescan button for more)
hangar scan     # import new sessions and print counts
hangar list     # print the project table
```

## Documentation

The docs follow [Diátaxis](https://diataxis.fr/); start at [docs/README.md](docs/README.md).

- **Tutorial:** [Getting started](docs/tutorials/getting-started.md)
- **How-to:** [Choose what Hangar scans](docs/how-to/configure-sources.md) · [Clean up](docs/how-to/clean-up.md) · [Change the theme](docs/how-to/change-theme.md) · [Make a release](docs/how-to/release.md) · [Add another AI tool](docs/how-to/add-an-ai-tool.md)
- **Reference:** [CLI](docs/reference/cli.md) · [Configuration](docs/reference/configuration.md) · [Project fields](docs/reference/project-fields.md)
- **Explanation:** [How Hangar works](docs/explanation/how-hangar-works.md)

## Roadmap

v1 scope is in [SPEC.md](SPEC.md). v2 plans: a tray icon and a background service. Later: more AI tools, merging data from several machines, and signed releases.

## License

[MIT](LICENSE)
