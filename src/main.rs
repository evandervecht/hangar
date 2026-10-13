// shortcut: console subsystem so `hangar scan|list` can print; a console window flashes when the GUI is double-clicked on Windows.
mod gui;
mod scan;

use serde::{Deserialize, Serialize};
use std::env;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize)]
pub struct Config {
    /// Folders scanned (4 levels deep) for git repos never opened with an AI tool.
    pub roots: Vec<String>,
    /// Whole path segments to hide, e.g. "Temp" or "plugins/cache".
    pub excludes: Vec<String>,
}

fn data_dir(home: &Path) -> PathBuf {
    let base = if cfg!(windows) {
        env::var_os("APPDATA").map_or_else(|| home.join("AppData").join("Roaming"), PathBuf::from)
    } else if cfg!(target_os = "macos") {
        home.join("Library").join("Application Support")
    } else {
        env::var_os("XDG_DATA_HOME")
            .map_or_else(|| home.join(".local").join("share"), PathBuf::from)
    };
    base.join("hangar")
}

fn load_config(path: &Path, home: &Path) -> Result<Config, Box<dyn Error>> {
    if let Ok(s) = fs::read_to_string(path) {
        return Ok(toml::from_str(&s)?);
    }
    let root = home.join("Projects");
    let cfg = Config {
        roots: vec![root.to_string_lossy().to_string()],
        excludes: ["Temp", "scratchpad", "plugins/cache"]
            .map(String::from)
            .to_vec(),
    };
    fs::write(path, toml::to_string(&cfg)?)?;
    Ok(cfg)
}

fn main() -> Result<(), Box<dyn Error>> {
    let home = env::home_dir().ok_or("no home directory")?;
    let dir = data_dir(&home);
    fs::create_dir_all(&dir)?;
    let cfg = load_config(&dir.join("config.toml"), &home)?;
    let mut db = scan::open(&dir.join("hangar.db"))?;

    match env::args().nth(1).as_deref() {
        None => {
            let s = scan::run(&mut db, &home, &cfg.roots, &cfg.excludes)?;
            let msg = format!("imported {} log files, {} projects", s.files, s.projects);
            gui::run(db, home, cfg, msg)?;
        }
        Some("scan") => {
            let s = scan::run(&mut db, &home, &cfg.roots, &cfg.excludes)?;
            println!(
                "imported {} log files, {} projects (data: {})",
                s.files,
                s.projects,
                dir.display()
            );
        }
        Some("list") => {
            println!(
                "{:<55} {:<8} {:<13} {:>8} {:>8} {:>7} {:<16} ORIGIN",
                "PROJECT", "STATUS", "TOOLS", "SESSIONS", "ACTIVE", "7 DAYS", "LAST (UTC)"
            );
            for r in scan::list(&db, &cfg.excludes)? {
                let name = if r.missing {
                    format!("{} (missing)", r.path)
                } else {
                    r.path
                };
                println!(
                    "{:<55} {:<8} {:<13} {:>8} {:>8} {:>7} {:<16} {}",
                    name,
                    r.status,
                    r.tools,
                    r.sessions,
                    gui::hours(r.active_secs),
                    gui::hours(r.week_secs),
                    r.last_active.get(..16).unwrap_or("").replace('T', " "),
                    r.origin
                );
            }
        }
        Some(_) => {
            eprintln!("usage: hangar [scan | list]   (no argument opens the window)");
            std::process::exit(2);
        }
    }
    Ok(())
}
