// Console subsystem so `hangar scan|list` print and pipes work; the window mode drops the console it was started with.
mod gui;
mod scan;

use serde::{Deserialize, Serialize};
use std::env;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Serialize, Deserialize)]
pub struct Config {
    /// Folders scanned (4 levels deep) for git repos never opened with an AI tool.
    pub roots: Vec<String>,
    /// Whole path segments to hide, e.g. "Temp" or "plugins/cache".
    pub excludes: Vec<String>,
    /// "system" (default), "dark", "light" or "hangar".
    #[serde(default)]
    pub theme: String,
    /// Keep running in the tray when the window is closed, and rescan on a timer.
    #[serde(default)]
    pub background: bool,
    /// Start in the tray at login (implies background).
    #[serde(default)]
    pub autostart: bool,
    /// Minutes between background scans.
    #[serde(default = "default_scan_minutes")]
    pub scan_minutes: u64,
}

fn default_scan_minutes() -> u64 {
    15
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
        theme: String::new(),
        background: false,
        autostart: false,
        scan_minutes: default_scan_minutes(),
    };
    save_config(path, &cfg)?;
    Ok(cfg)
}

pub fn save_config(path: &Path, cfg: &Config) -> Result<(), Box<dyn Error>> {
    Ok(fs::write(path, toml::to_string(cfg)?)?)
}

/// Start (or stop) Hangar in the tray at login: HKCU Run key, LaunchAgent, or XDG autostart entry.
pub fn set_autostart(enabled: bool) -> Result<(), String> {
    let exe = env::current_exe().map_err(|e| e.to_string())?;
    let home = env::home_dir().ok_or("no home directory")?;
    if cfg!(windows) {
        let key = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
        let mut cmd = scan::quiet_cmd("reg");
        if enabled {
            let value = format!("\"{}\" --tray", exe.display());
            cmd.args([
                "add", key, "/v", "Hangar", "/t", "REG_SZ", "/d", &value, "/f",
            ]);
        } else {
            cmd.args(["delete", key, "/v", "Hangar", "/f"]);
        }
        let out = cmd.output().map_err(|e| e.to_string())?;
        // Deleting a value that isn't there is fine.
        if out.status.success() || !enabled {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
        }
    } else {
        let (file, body) = if cfg!(target_os = "macos") {
            (
                home.join("Library/LaunchAgents/io.github.evandervecht.hangar.plist"),
                format!(
                    r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>io.github.evandervecht.hangar</string>
  <key>ProgramArguments</key><array><string>{}</string><string>--tray</string></array>
  <key>RunAtLoad</key><true/>
</dict>
</plist>
"#,
                    exe.display()
                ),
            )
        } else {
            (
                home.join(".config/autostart/hangar.desktop"),
                format!(
                    "[Desktop Entry]\nType=Application\nName=Hangar\nExec=\"{}\" --tray\n",
                    exe.display()
                ),
            )
        };
        if enabled {
            fs::create_dir_all(file.parent().expect("autostart file has a folder"))
                .and_then(|()| fs::write(&file, body))
                .map_err(|e| e.to_string())
        } else {
            match fs::remove_file(&file) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
                _ => Ok(()),
            }
        }
    }
}

/// When started from Explorer / the Start Menu, Windows gives this console app its own console window;
/// drop it so only the Hangar window shows. From a terminal the console is shared and stays.
fn release_own_console() {
    #[cfg(windows)]
    {
        unsafe extern "system" {
            fn GetConsoleProcessList(list: *mut u32, count: u32) -> u32;
            fn FreeConsole() -> i32;
        }
        let mut pids = [0u32; 2];
        // SAFETY: plain kernel32 calls; the buffer outlives the call and its length is passed.
        unsafe {
            if GetConsoleProcessList(pids.as_mut_ptr(), pids.len() as u32) == 1 {
                FreeConsole();
            }
        }
    }
}

fn run_window(
    dir: &Path,
    home: PathBuf,
    cfg: Config,
    cfg_path: PathBuf,
    tray: bool,
) -> Result<(), Box<dyn Error>> {
    // One window per user: a second start asks the running one to show itself, then exits.
    let lock = fs::File::create(dir.join("hangar.lock"))?;
    if lock.try_lock().is_err() {
        fs::write(dir.join("show-request"), "")?;
        return Ok(());
    }
    gui::run(dir.to_path_buf(), home, cfg, cfg_path, tray)?;
    drop(lock);
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let home = env::home_dir().ok_or("no home directory")?;
    let dir = data_dir(&home);
    fs::create_dir_all(&dir)?;
    let cfg_path = dir.join("config.toml");
    let cfg = load_config(&cfg_path, &home)?;

    let arg = env::args().nth(1);
    if arg.is_none() || arg.as_deref() == Some("--tray") {
        release_own_console();
        // Without a console, a failed start would be silent: leave the reason in the data folder.
        if let Err(e) = run_window(&dir, home, cfg, cfg_path, arg.is_some()) {
            let _ = fs::write(
                dir.join("hangar.log"),
                format!("Hangar failed to start: {e}\n"),
            );
            return Err(e);
        }
        return Ok(());
    }
    let mut db = scan::open(&dir.join("hangar.db"))?;

    match arg.as_deref() {
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
            let rows = scan::list(&db, &cfg.excludes)?;
            match env::args().nth(2).as_deref() {
                Some("--json") => println!("{}", serde_json::to_string_pretty(&rows)?),
                Some("--csv") => print!("{}", scan::csv(&rows)),
                Some(_) => {
                    eprintln!("usage: hangar list [--json | --csv]");
                    std::process::exit(2);
                }
                None => {
                    println!(
                        "{:<55} {:<8} {:<20} {:<13} {:>8} {:>8} {:>7} {:<16} ORIGIN",
                        "PROJECT",
                        "STATUS",
                        "GIT",
                        "TOOLS",
                        "SESSIONS",
                        "ACTIVE",
                        "7 DAYS",
                        "LAST (UTC)"
                    );
                    for r in rows {
                        let name = if r.missing {
                            format!("{} (missing)", r.path)
                        } else {
                            r.path.clone()
                        };
                        println!(
                            "{:<55} {:<8} {:<20} {:<13} {:>8} {:>8} {:>7} {:<16} {}",
                            name,
                            r.shown_status,
                            scan::git_label(&r),
                            r.tools,
                            r.sessions,
                            gui::hours(r.active_secs),
                            gui::hours(r.week_secs),
                            r.last_active.get(..16).unwrap_or("").replace('T', " "),
                            r.origin
                        );
                    }
                }
            }
        }
        _ => {
            eprintln!(
                "usage: hangar [scan | list [--json | --csv] | --tray]   (no argument opens the window)"
            );
            std::process::exit(2);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    /// Touches the real user's login items, so it only runs on request: `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn autostart_roundtrip() {
        let registered = || {
            if cfg!(windows) {
                crate::scan::quiet_cmd("reg")
                    .args([
                        "query",
                        r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
                        "/v",
                        "Hangar",
                    ])
                    .output()
                    .unwrap()
                    .status
                    .success()
            } else if cfg!(target_os = "macos") {
                std::env::home_dir()
                    .unwrap()
                    .join("Library/LaunchAgents/io.github.evandervecht.hangar.plist")
                    .exists()
            } else {
                std::env::home_dir()
                    .unwrap()
                    .join(".config/autostart/hangar.desktop")
                    .exists()
            }
        };
        crate::set_autostart(true).unwrap();
        assert!(registered());
        crate::set_autostart(false).unwrap();
        assert!(!registered());
        crate::set_autostart(false).unwrap(); // removing twice is fine
    }
}
