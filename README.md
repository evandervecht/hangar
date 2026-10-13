<p align="center"><img src="assets/hangar.png" width="160" alt="Hangar logo"></p>

# Hangar

[![CI](https://github.com/evandervecht/hangar/actions/workflows/ci.yml/badge.svg)](https://github.com/evandervecht/hangar/actions/workflows/ci.yml)

A CMDB for your AI coding projects. Hangar tracks where your Claude Code and Codex projects live, what they are, their git origin and the time you've spent in each. It's built in Rust for Windows and macOS.

## What it does

- **Finds your projects** from the session logs Claude Code (`~/.claude/projects`) and Codex (`~/.codex/sessions`) leave behind, plus git repos under your work folders.
- **Shows what each one is.** The description comes from the first paragraph of `README.md`, `CLAUDE.md` or `AGENTS.md`.
- **Shows where it lives and where it syncs to:** the path and the git origin, or "local only" / "no git".
- **Shows the time spent:** sessions, active time (total and the last 7 days, with idle gaps over 15 minutes left out) and when the project was last active.
- **Lets you add your own fields:** tags, a status (active / paused / archived) and notes.
- **Never forgets:** a project whose folder is gone stays in the list, flagged *missing*.

Everything stays on your machine. Hangar only reads the logs and writes its own local SQLite file.

## Install

**Download:** grab the zip (Windows) or tar.gz (macOS) from [Releases](https://github.com/evandervecht/hangar/releases). The binaries are unsigned; see [development.md](docs/development.md#release) for the macOS quarantine step.

**From source:** you need a [Rust toolchain](https://rustup.rs).

```sh
git clone https://github.com/evandervecht/hangar.git
cd hangar
./install.ps1   # Windows: installs to %LOCALAPPDATA%\Programs\hangar and adds a Start Menu shortcut
./install.sh    # macOS / Linux: installs to ~/.local/bin
```

## Use

```
hangar          # open the window (scans on launch, Rescan button for more)
hangar scan     # import new sessions and print counts
hangar list     # print the project table
```

## Config

Hangar creates `config.toml` on first run, next to its database:

| OS      | Folder                                  |
|---------|-----------------------------------------|
| Windows | `%APPDATA%\hangar`                      |
| macOS   | `~/Library/Application Support/hangar`  |
| Linux   | `~/.local/share/hangar`                 |

```toml
roots = ['C:\Users\you\Projects']                  # default ~/Projects, scanned 4 levels deep for git repos
excludes = ["Temp", "scratchpad", "plugins/cache"] # whole path segments to hide
```

## Documentation

- [How it works](docs/how-it-works.md): sources, project identity, active time, storage
- [Configuration](docs/configuration.md): data folder, roots, excludes
- [Development](docs/development.md): build, test, release, adding another AI tool

## Roadmap

v1 scope is in [SPEC.md](SPEC.md). v2 plans: a tray icon and a background service. Later: more AI tools, merging data from several machines, and signed releases.

## License

[MIT](LICENSE)
