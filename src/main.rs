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
    #[serde(default = "default_roots")]
    pub roots: Vec<String>,
    /// Whole path segments to hide, e.g. "Temp" or "plugins/cache".
    #[serde(default = "default_excludes")]
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

fn default_roots() -> Vec<String> {
    let home = env::home_dir().unwrap_or_default();
    vec![home.join("Projects").to_string_lossy().to_string()]
}

fn default_excludes() -> Vec<String> {
    ["Temp", "scratchpad", "plugins/cache"]
        .map(String::from)
        .to_vec()
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

/// Reads config.toml; only a missing file is created with defaults, any other error leaves the file alone.
fn load_config(path: &Path) -> Result<Config, Box<dyn Error>> {
    match fs::read_to_string(path) {
        Ok(s) => Ok(toml::from_str(&s)?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let cfg: Config = toml::from_str("")?; // every key takes its default
            save_config(path, &cfg)?;
            Ok(cfg)
        }
        Err(e) => Err(format!("can't read {}: {e}", path.display()).into()),
    }
}

/// Writes a temp file and renames it over the config, so a crash mid-write never leaves half a file.
pub fn save_config(path: &Path, cfg: &Config) -> Result<(), Box<dyn Error>> {
    let tmp = path.with_extension("toml.tmp");
    fs::write(&tmp, toml::to_string(cfg)?)?;
    Ok(fs::rename(tmp, path)?)
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
                    // launchd ignores a plist that isn't valid XML, e.g. a path with "&".
                    exe.display()
                        .to_string()
                        .replace('&', "&amp;")
                        .replace('<', "&lt;")
                ),
            )
        } else {
            // shortcut: Exec quoting isn't escaped (", $, `, \ in the exe path); fine for normal install paths.
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

const USAGE: &str = "usage: hangar [scan | list [--json | --csv] | --tray | --version | --help]   (no argument opens the window)";

fn main() -> Result<(), Box<dyn Error>> {
    let arg = env::args().nth(1);
    match arg.as_deref() {
        Some("--version" | "-V") => {
            println!("hangar {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        Some("--help" | "-h") => {
            println!("{USAGE}");
            return Ok(());
        }
        _ => {}
    }
    let home = env::home_dir().ok_or("no home directory")?;
    let dir = data_dir(&home);
    let window = matches!(arg.as_deref(), None | Some("--tray"));
    let result = start(home, &dir, arg.as_deref(), window);
    // Without a console, a failed window start (bad config.toml, locked database) would be silent:
    // leave the reason in the data folder.
    if window && let Err(e) = &result {
        let _ = fs::write(
            dir.join("hangar.log"),
            format!(
                "Hangar failed to start: {e}
"
            ),
        );
    }
    result
}

fn start(home: PathBuf, dir: &Path, arg: Option<&str>, window: bool) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(dir)?;
    let cfg_path = dir.join("config.toml");
    let cfg = load_config(&cfg_path)?;

    if window {
        release_own_console();
        // Re-register on each start, so the login entry follows the exe if it was moved. Not from a
        // cargo build (target/) or a quarantined macOS copy (AppTranslocation): those paths don't last.
        let exe = env::current_exe().unwrap_or_default();
        let lasting = !exe
            .components()
            .any(|c| c.as_os_str() == "target" || c.as_os_str() == "AppTranslocation");
        if cfg.autostart && lasting {
            let _ = set_autostart(true);
        }
        return run_window(dir, home, cfg, cfg_path, arg.is_some());
    }
    let mut db = scan::open(&dir.join("hangar.db"))?;

    match arg {
        Some("scan") => {
            let s = scan::run(
                &mut db,
                &home,
                &scan::hermes_home(&home),
                &cfg.roots,
                &cfg.excludes,
            )?;
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
            eprintln!("{USAGE}");
            std::process::exit(2);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn config_fills_gaps_and_saves_whole() {
        let cfg: crate::Config = toml::from_str("theme = \"dark\"").unwrap();
        assert_eq!(cfg.excludes, crate::default_excludes());
        assert!(cfg.roots[0].ends_with("Projects"));
        assert_eq!(cfg.scan_minutes, 15);
        let path = std::env::temp_dir().join("hangar-config-test.toml");
        // A file that isn't UTF-8 is an error, never replaced by defaults.
        std::fs::write(&path, b"\xff").unwrap();
        assert!(crate::load_config(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"\xff");
        crate::save_config(&path, &cfg).unwrap();
        let back: crate::Config = toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(back.theme, "dark");
        assert!(!path.with_extension("toml.tmp").exists());
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn scan_minutes_is_clamped_on_load() {
        let path = std::env::temp_dir().join("hangar-clamp-test.toml");
        for (raw, want) in [(0, 1), (999_999_999_999_u64, 1440), (30, 30)] {
            std::fs::write(&path, format!("scan_minutes = {raw}")).unwrap();
            assert_eq!(crate::load_config(&path).unwrap().scan_minutes, want);
        }
        std::fs::remove_file(path).unwrap();
    }

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
