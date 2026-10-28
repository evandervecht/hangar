use crate::Config;
use crate::scan::{self, Row, Scratchpad};
use eframe::egui;
use rusqlite::Connection;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
#[cfg(any(windows, target_os = "macos"))]
use std::sync::mpsc::{Receiver, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

const THEMES: [(&str, &str); 4] = [
    ("", "System"),
    ("dark", "Dark"),
    ("light", "Light"),
    ("hangar", "Hangar"),
];

const UNKNOWN_GIT: &str = "Git couldn't read this repo, often 'dubious ownership' (the folder belongs to another user). Run `git status` in it to see why.";

/// Table columns and what each means, shown when hovering the header (docs/reference/project-fields.md).
const COLUMNS: [(&str, &str); 14] = [
    (
        "Project",
        "Folder name; (missing) when the folder no longer exists.",
    ),
    ("Location", "The folder the project lives in."),
    (
        "Status",
        "active: a session ran in the last 14 days. idle: none since.\n\
         active / paused / archived set by you in the detail panel always win.",
    ),
    (
        "Git",
        "clean: has a remote, nothing changed, unpushed or stashed.\n\
         N changed: changed or untracked files.\n\
         N unpushed: commits on any local branch that no remote has.\n\
         N stashed: entries in git stash.\n\
         local only: a repo without an origin remote.\n\
         no git: not a git repo.\n\
         ?: git couldn't read the repo.\n\
         empty: the folder is gone.\n\
         Everything but clean, ? and empty is at risk.",
    ),
    (
        "Stack",
        "Languages and tools, from marker files in the project folder.",
    ),
    (
        "Clones",
        "Other projects that are checkouts of the same repo.",
    ),
    (
        "Size",
        "Disk used by the files in the project folder, node_modules and target included. Symlinks aren't followed.",
    ),
    ("Tags", "Your own labels, set in the detail panel."),
    (
        "Tools",
        "AI tools with sessions here: claude, codex, gemini, hermes.",
    ),
    ("Sessions", "Number of AI sessions."),
    (
        "Active",
        "Total active time; gaps over 15 minutes don't count.",
    ),
    ("7 days", "Active time today and the 6 days before (UTC)."),
    (
        "Last active (UTC)",
        "Time of the most recent session activity.",
    ),
    (
        "Description",
        "First paragraph of README.md, else CLAUDE.md, else AGENTS.md.",
    ),
];

/// Manual statuses; "" means derived from activity.
const STATUSES: [&str; 4] = ["", "active", "paused", "archived"];

struct App {
    db: Connection,
    home: PathBuf,
    cfg: Config,
    cfg_path: PathBuf,
    rows: Vec<Row>,
    /// Claude Code scratchpads per project path, shown as sub-rows of expanded projects.
    scratch: HashMap<String, Vec<Scratchpad>>,
    expanded: HashSet<String>,
    filter: String,
    at_risk_only: bool,
    selected: Option<Row>,
    /// The selected project's sessions that can be reopened; read when it's selected.
    resumable: Vec<scan::Resumable>,
    full_text: String,
    message: String,
    show_clean: bool,
    log_days: u64,
    old_logs: Option<(Vec<PathBuf>, u64)>,
    confirm_name: String,
    /// Copy of the config being edited in Settings; None when the window is closed.
    settings: Option<Config>,
    data_dir: PathBuf,
    /// The running scan; one at a time, shared by launch, Rescan, Settings and the background timer.
    scan: Option<JoinHandle<Result<scan::Stats, String>>>,
    last_scan: Instant,
    /// "Check for updates" in Settings: the running check, then the latest version or why it failed.
    update_check: Option<JoinHandle<Result<String, String>>>,
    update: Option<Result<String, String>>,
    /// Started with --tray: background mode for this session even if the setting is off.
    tray_start: bool,
    hidden: bool,
    quitting: bool,
    #[cfg(any(windows, target_os = "macos"))]
    tray: Option<tray_icon::TrayIcon>,
    /// Clicks in the tray menu, on the tray icon and (macOS) in the app menu.
    #[cfg(any(windows, target_os = "macos"))]
    menu: (Sender<tray::Cmd>, Receiver<tray::Cmd>),
    /// Creating the tray icon failed once; don't retry every frame.
    #[cfg(any(windows, target_os = "macos"))]
    tray_failed: bool,
}

enum Action {
    Save,
    Hide,
    Forget,
    TrashFolder,
    /// Merge the selected (missing) project into this path.
    Merge(String),
}

pub fn run(
    data_dir: PathBuf,
    home: PathBuf,
    cfg: Config,
    cfg_path: PathBuf,
    tray_start: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let db = scan::open(&data_dir.join("hangar.db"))?;
    let mut app = App {
        db,
        home,
        cfg,
        cfg_path,
        rows: Vec::new(),
        scratch: HashMap::new(),
        expanded: HashSet::new(),
        filter: String::new(),
        at_risk_only: false,
        selected: None,
        resumable: Vec::new(),
        full_text: String::new(),
        message: String::new(),
        show_clean: false,
        log_days: 90,
        old_logs: None,
        confirm_name: String::new(),
        settings: None,
        data_dir,
        scan: None,
        last_scan: Instant::now(),
        update_check: None,
        update: None,
        tray_start,
        hidden: tray_start,
        quitting: false,
        #[cfg(any(windows, target_os = "macos"))]
        tray: None,
        #[cfg(any(windows, target_os = "macos"))]
        menu: std::sync::mpsc::channel(),
        #[cfg(any(windows, target_os = "macos"))]
        tray_failed: false,
    };
    // Show the last scan's rows at once; the launch scan runs in the background.
    app.reload();
    app.start_scan();
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 800.0])
            .with_icon(
                eframe::icon_data::from_png_bytes(include_bytes!("../assets/hangar.png"))
                    .expect("bundled icon is a valid png"),
            ),
        ..Default::default()
    };
    eframe::run_native(
        "Hangar",
        opts,
        Box::new(|cc| {
            apply_theme(&cc.egui_ctx, &app.cfg.theme);
            #[cfg(any(windows, target_os = "macos"))]
            tray::listen(&cc.egui_ctx, app.menu.0.clone());
            #[cfg(target_os = "macos")]
            tray::app_menu();
            Ok(Box::new(app))
        }),
    )?;
    Ok(())
}

impl App {
    // Rows are loaded on launch/rescan/save only; egui repaints often and the list query isn't free.
    fn reload(&mut self) {
        match scan::list(&self.db, &self.cfg.excludes) {
            Ok(rows) => self.rows = rows,
            Err(e) => self.message = format!("load failed: {e}"),
        }
        self.scratch = scan::scratchpads(&self.db, &std::env::temp_dir());
        // A rescan adds sessions, Clean trashes their logs.
        if let Some(sel) = &self.selected {
            self.resumable = scan::resumable(&self.db, &sel.path, 5);
        }
    }

    fn select(&mut self, row: Row) {
        self.full_text = scan::description_file(Path::new(&row.path))
            .and_then(|f| std::fs::read_to_string(f).ok())
            .unwrap_or_else(|| "(no README.md, CLAUDE.md or AGENTS.md)".into());
        self.confirm_name.clear();
        self.resumable = scan::resumable(&self.db, &row.path, 5);
        self.selected = Some(row);
    }

    fn apply(&mut self, action: Action) {
        let Some(sel) = self.selected.clone() else {
            return;
        };
        self.message = match action {
            Action::Save => match scan::save_manual(&self.db, &sel) {
                Ok(()) => "saved".into(),
                Err(e) => format!("save failed: {e}"),
            },
            Action::Hide => {
                self.cfg.excludes.push(sel.path.clone());
                // An open Settings window saves its copy later; it must not bring the project back.
                if let Some(draft) = self.settings.as_mut() {
                    draft.excludes.push(sel.path.clone());
                }
                self.selected = None;
                match crate::save_config(&self.cfg_path, &self.cfg) {
                    Ok(()) => format!("hidden {} (added to excludes)", sel.path),
                    Err(e) => format!("hidden for now, config save failed: {e}"),
                }
            }
            Action::Forget => {
                self.selected = None;
                match scan::forget(&mut self.db, std::slice::from_ref(&sel.path)) {
                    Ok(_) => format!("forgot {}", sel.path),
                    Err(e) => format!("forget failed: {e}"),
                }
            }
            Action::TrashFolder => {
                match scan::trash_folder(&sel.path, &self.home, &self.cfg.roots, &self.data_dir) {
                    Ok(()) => {
                        self.selected = None;
                        let _ = self.db.execute(
                            "UPDATE projects SET missing = 1 WHERE path = ?1",
                            [&sel.path],
                        );
                        format!("moved {} to the Recycle Bin / Trash", sel.path)
                    }
                    Err(e) => format!("delete failed: {e}"),
                }
            }
            Action::Merge(to) => match scan::merge(&mut self.db, &sel.path, &to) {
                Ok(()) => {
                    self.selected = None;
                    format!("merged {} into {to}", sel.path)
                }
                Err(e) => format!("merge failed: {e}"),
            },
        };
        self.reload();
    }

    /// Scan on a background thread with its own connection; the window keeps responding.
    fn start_scan(&mut self) {
        if self.scanning() {
            return;
        }
        let db_path = self.data_dir.join("hangar.db");
        let (home, roots, excludes) = (
            self.home.clone(),
            self.cfg.roots.clone(),
            self.cfg.excludes.clone(),
        );
        self.scan = Some(std::thread::spawn(move || {
            let mut db = scan::open(&db_path).map_err(|e| e.to_string())?;
            scan::run(&mut db, &home, &scan::hermes_home(&home), &roots, &excludes)
                .map_err(|e| e.to_string())
        }));
        self.last_scan = Instant::now();
        self.message = "scanning...".into();
    }

    fn scanning(&self) -> bool {
        self.scan.is_some()
    }

    fn poll_scan(&mut self) {
        if !self.scan.as_ref().is_some_and(|h| h.is_finished()) {
            return;
        }
        let result = self
            .scan
            .take()
            .expect("checked above")
            .join()
            .unwrap_or_else(|_| Err("scan crashed".into()));
        self.message = match result {
            Ok(s) => format!("imported {} log files, {} projects", s.files, s.projects),
            Err(e) => format!("scan failed: {e}"),
        };
        self.reload();
    }

    /// Keep running when the window closes, and rescan on a timer.
    fn background(&self) -> bool {
        self.cfg.background || self.cfg.autostart || self.tray_start
    }

    /// Hiding the window is only safe with a tray icon to bring it back (none on Linux, or if it failed).
    fn has_tray(&self) -> bool {
        #[cfg(any(windows, target_os = "macos"))]
        return self.tray.is_some();
        #[cfg(not(any(windows, target_os = "macos")))]
        false
    }

    fn show(&mut self, ctx: &egui::Context) {
        self.hidden = false;
        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
    }

    fn open_settings(&mut self) {
        if self.settings.is_none() {
            self.settings = Some(self.cfg.clone());
        }
    }

    fn check_updates(&mut self) {
        if self.update_check.is_none() {
            self.update = None;
            self.update_check = Some(std::thread::spawn(latest_release));
        }
    }

    /// Really quit, also in background mode (closing the window would only hide it).
    fn quit(&mut self, ctx: &egui::Context) {
        self.quitting = true;
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    fn settings_window(&mut self, ctx: &egui::Context) {
        let scanning = self.scanning();
        let Some(draft) = self.settings.as_mut() else {
            return;
        };
        let (mut save, mut cancel, mut check) = (false, false, false);
        let (checking, update) = (self.update_check.is_some(), &self.update);
        let mut open = true;
        egui::Window::new("Settings")
            .open(&mut open)
            .resizable(false)
            .min_width(600.0)
            .enabled(!scanning)
            .show(ctx, |ui| {
                ui.strong("Project folders");
                ui.weak("Scanned 4 levels deep for git repos, next to the Claude Code and Codex logs.");
                edit_list(ui, &mut draft.roots, true);
                ui.separator();
                ui.strong("Hidden paths");
                ui.weak("Whole folder names (Temp) or full paths. \"Hide from Hangar\" adds to this list.");
                edit_list(ui, &mut draft.excludes, false);
                ui.separator();
                ui.strong("Background");
                ui.checkbox(&mut draft.background, "Keep running in the tray when the window is closed");
                ui.checkbox(&mut draft.autostart, "Start Hangar in the tray when I log in");
                ui.horizontal(|ui| {
                    ui.label("Rescan in the background every");
                    ui.add(egui::DragValue::new(&mut draft.scan_minutes).range(1..=1440));
                    ui.label("minutes");
                });
                ui.separator();
                ui.horizontal(|ui| {
                    save = ui.button("Save and rescan").clicked();
                    cancel = ui.button("Cancel").clicked();
                });
                ui.separator();
                ui.horizontal(|ui| {
                    ui.weak(format!("Hangar {VERSION}"));
                    if checking {
                        ui.spinner();
                    } else if ui.button("Check for updates").clicked() {
                        check = true;
                    }
                    match update {
                        Some(Ok(v)) if newer(v, VERSION) => {
                            ui.hyperlink_to(format!("Hangar {v} is available"), RELEASES_URL);
                        }
                        Some(Ok(_)) => {
                            ui.label("You have the latest version");
                        }
                        Some(Err(e)) => {
                            ui.label(format!("Couldn't check: {e}"));
                        }
                        None => {}
                    }
                });
            });
        if check {
            self.check_updates();
        }
        if save {
            let mut cfg = self.settings.take().expect("settings open");
            for list in [&mut cfg.roots, &mut cfg.excludes] {
                list.iter_mut().for_each(|s| *s = s.trim().to_string());
                list.retain(|s| !s.is_empty());
            }
            // Starting at login only makes sense in the tray.
            cfg.background |= cfg.autostart;
            // The theme is picked outside Settings; keep a change made while this window was open.
            cfg.theme.clone_from(&self.cfg.theme);
            // Register first, so the saved setting never claims a login start that isn't there.
            if cfg.autostart != self.cfg.autostart
                && let Err(e) = crate::set_autostart(cfg.autostart)
            {
                self.message = format!("start at login not changed: {e}");
                cfg.autostart = self.cfg.autostart;
            }
            self.cfg = cfg;
            if let Err(e) = crate::save_config(&self.cfg_path, &self.cfg) {
                self.message = format!("settings not saved: {e}");
                return;
            }
            self.start_scan();
        } else if cancel || !open {
            self.settings = None;
        }
    }

    fn clean_window(&mut self, ctx: &egui::Context) {
        let mut open = self.show_clean;
        let missing = self.rows.iter().filter(|r| r.missing).count();
        egui::Window::new("Clean up")
            .open(&mut open)
            .resizable(false)
            .enabled(!self.scanning())
            .show(ctx, |ui| {
                ui.strong("Missing projects");
                ui.label(format!(
                    "{missing} projects whose folder is gone. Forgetting removes them and their history from Hangar."
                ));
                if ui
                    .add_enabled(missing > 0, egui::Button::new("Forget all missing"))
                    .clicked()
                {
                    // Exactly the rows counted above; hidden (excluded) projects stay.
                    let paths: Vec<String> = self
                        .rows
                        .iter()
                        .filter(|r| r.missing)
                        .map(|r| r.path.clone())
                        .collect();
                    self.message = match scan::forget(&mut self.db, &paths) {
                        Ok(n) => format!("forgot {n} missing projects"),
                        Err(e) => format!("forget failed: {e}"),
                    };
                    self.selected = None;
                    self.reload();
                }

                ui.separator();
                ui.strong("Old AI session logs");
                ui.horizontal(|ui| {
                    ui.label("Not touched for");
                    if ui
                        .add(egui::DragValue::new(&mut self.log_days).range(7..=3650))
                        .changed()
                    {
                        self.old_logs = None;
                    }
                    ui.label("days");
                    if ui.button("Find").clicked() {
                        self.old_logs = Some(scan::old_logs(
                            &self.home,
                            &scan::hermes_home(&self.home),
                            self.log_days,
                        ));
                    }
                });
                if let Some((files, size)) = &self.old_logs {
                    ui.label(format!(
                        "{} AI session log files, {:.1} MB. Hangar keeps their imported history.",
                        files.len(),
                        *size as f64 / 1e6
                    ));
                    let label = format!("Move {} files to the Recycle Bin / Trash", files.len());
                    if ui
                        .add_enabled(!files.is_empty(), egui::Button::new(label))
                        .clicked()
                    {
                        self.message = match scan::trash_logs(files, self.log_days) {
                            Ok(n) => format!("moved {n} log files to the Recycle Bin / Trash"),
                            Err(e) => format!("trash failed: {e}"),
                        };
                        self.old_logs = None;
                    }
                }
            });
        self.show_clean = open;
    }
}

impl App {
    /// The selected project: facts, manual fields and clean-up actions. Returns the action clicked
    /// and a clone clicked (to select it).
    fn detail_panel(&mut self, ui: &mut egui::Ui, scanning: bool) -> (Option<Action>, Option<Row>) {
        let (mut action, mut clicked) = (None, None);
        let Some(sel) = self.selected.as_mut() else {
            return (None, None);
        };
        let guard = scan::delete_guard(&sel.path, &self.home, &self.cfg.roots, &self.data_dir);
        let key = scan::origin_key(&sel.origin);
        let clones: Vec<&Row> = self
            .rows
            .iter()
            .filter(|r| r.path != sel.path && key.is_some() && scan::origin_key(&r.origin) == key)
            .collect();
        // A moved project looks like a new one with the same repo (or, without git, the same folder name).
        let move_targets: Vec<&Row> = if sel.missing {
            self.rows
                .iter()
                .filter(|r| !r.missing && r.path != sel.path)
                .filter(|r| match &key {
                    Some(k) => scan::origin_key(&r.origin).as_ref() == Some(k),
                    None => r.name == sel.name,
                })
                .collect()
        } else {
            vec![]
        };
        egui::Panel::right("detail")
            .resizable(true)
            .default_size(450.0)
            .show(ui, |ui| {
                // While a scan writes, edits wait: SQLite has one writer.
                ui.add_enabled_ui(!scanning, |ui| {
                ui.heading(&sel.name);
                ui.label(&sel.path);
                ui.label(&sel.origin);
                ui.label(format!(
                    "{} sessions ({}), first seen {}",
                    sel.sessions,
                    sel.tools,
                    sel.first_seen.get(..10).unwrap_or("")
                ));
                let git = scan::git_label(sel);
                let last = sel.git_last_commit.as_deref().and_then(|t| t.get(..10));
                let git_line = match last {
                    Some(d) => format!("Git: {git}, last commit {d}"),
                    None => format!("Git: {git}"),
                };
                if sel.at_risk {
                    ui.colored_label(ui.visuals().warn_fg_color, format!("{git_line} (at risk)"));
                } else {
                    ui.label(git_line);
                }
                if !sel.stack.is_empty() {
                    ui.label(format!("Stack: {}", sel.stack));
                }
                if !clones.is_empty() {
                    ui.collapsing(format!("Other clones ({})", clones.len()), |ui| {
                        for c in &clones {
                            if ui.link(&c.path).clicked() {
                                clicked = Some((*c).clone());
                            }
                        }
                    });
                }
                if sel.missing {
                    ui.separator();
                    if move_targets.is_empty() {
                        ui.weak("Folder is gone. No project with the same repo or name found to merge into.");
                    } else {
                        ui.label("Folder is gone. If it moved, merge its history into the new location:");
                        for t in &move_targets {
                            if ui
                                .button(format!("Merge into {}", t.path))
                                .on_hover_text("Moves sessions, fills empty tags/status/notes, and maps the old path to this one")
                                .clicked()
                            {
                                action = Some(Action::Merge(t.path.clone()));
                            }
                        }
                    }
                }
                ui.horizontal(|ui| {
                    if ui.button("Open folder").clicked() {
                        open(&sel.path);
                    }
                    if let Some(url) = browser_url(&sel.origin)
                        && ui.button("Open origin").clicked()
                    {
                        ui.ctx().open_url(egui::OpenUrl::new_tab(url));
                    }
                    if ui.button("Copy path").clicked() {
                        ui.ctx().copy_text(sel.path.clone());
                    }
                });
                if !self.resumable.is_empty() {
                    ui.collapsing("Resume a session", |ui| {
                        for s in &self.resumable {
                            let tool = if s.tool == "claude" { "Claude Code" } else { "Codex" };
                            if ui
                                .button(format!("{tool}, {}", s.last))
                                .on_hover_text(format!("Opens a terminal in {}", s.cwd))
                                .clicked()
                            {
                                self.message = match resume(s) {
                                    Ok(()) => format!("resuming {tool} session {}", s.session),
                                    Err(e) => format!("resume failed: {e}"),
                                };
                            }
                        }
                    });
                }
                ui.separator();
                egui::Grid::new("manual").num_columns(2).show(ui, |ui| {
                    ui.label("Tags");
                    ui.text_edit_singleline(&mut sel.tags);
                    ui.end_row();
                    ui.label("Status");
                    let auto = format!("auto ({})", sel.shown_status);
                    let shown = |s: &str| if s.is_empty() { auto.clone() } else { s.into() };
                    egui::ComboBox::from_id_salt("status")
                        .selected_text(shown(&sel.status))
                        .show_ui(ui, |ui| {
                            for s in STATUSES {
                                ui.selectable_value(&mut sel.status, s.to_string(), shown(s));
                            }
                        });
                    ui.end_row();
                    ui.label("Notes");
                    ui.text_edit_multiline(&mut sel.notes);
                    ui.end_row();
                });
                if ui.button("Save").clicked() {
                    action = Some(Action::Save);
                }

                ui.collapsing("Clean", |ui| {
                    if ui
                        .button("Hide from Hangar")
                        .on_hover_text("Adds this path to excludes in config.toml")
                        .clicked()
                    {
                        action = Some(Action::Hide);
                    }
                    if ui
                        .add_enabled(sel.missing, egui::Button::new("Forget"))
                        .on_hover_text("Removes this project and its history from Hangar")
                        .on_disabled_hover_text(
                            "Only for missing projects; use Hide for existing ones",
                        )
                        .clicked()
                    {
                        action = Some(Action::Forget);
                    }
                    ui.separator();
                    match guard {
                        Some(why) => {
                            ui.weak(format!("Folder can't be deleted: {why}."));
                        }
                        None => {
                            if sel.at_risk {
                                ui.colored_label(
                                    ui.visuals().warn_fg_color,
                                    format!("This folder holds work that isn't pushed anywhere ({}).", scan::git_label(sel)),
                                );
                            }
                            ui.label(format!(
                                "Delete the folder: type \"{}\" to confirm.",
                                sel.name
                            ));
                            ui.text_edit_singleline(&mut self.confirm_name);
                            if ui
                                .add_enabled(
                                    self.confirm_name == sel.name,
                                    egui::Button::new("Move folder to Recycle Bin / Trash"),
                                )
                                .clicked()
                            {
                                action = Some(Action::TrashFolder);
                            }
                        }
                    }
                });

                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| ui.monospace(&self.full_text));
                });
            });
        (action, clicked)
    }
}

impl eframe::App for App {
    // Runs before every frame and, while the window is hidden, on each requested repaint.
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_scan();
        if self.update_check.as_ref().is_some_and(|h| h.is_finished()) {
            let h = self.update_check.take().expect("checked above");
            self.update = Some(h.join().unwrap_or_else(|_| Err("check crashed".into())));
        }
        let background = self.background();
        #[cfg(any(windows, target_os = "macos"))]
        {
            if background && self.tray.is_none() && !self.tray_failed {
                self.tray = tray::create(ctx, self.menu.0.clone());
                self.tray_failed = self.tray.is_none();
            } else if !background {
                self.tray = None;
                self.tray_failed = false;
            }
            let cmds: Vec<tray::Cmd> = self.menu.1.try_iter().collect();
            for cmd in cmds {
                match cmd {
                    tray::Cmd::Show => self.show(ctx),
                    tray::Cmd::Rescan => self.start_scan(),
                    tray::Cmd::Quit => self.quit(ctx),
                    #[cfg(target_os = "macos")]
                    tray::Cmd::Settings => {
                        self.show(ctx);
                        self.open_settings();
                    }
                }
            }
        }
        // On macOS the app menu usually takes ⌘, before egui sees it.
        if ctx.input_mut(|i| i.consume_shortcut(&SETTINGS_KEY)) {
            self.open_settings();
        }
        // A second `hangar` start leaves this file and exits; bring this window forward instead.
        let request = self.data_dir.join("show-request");
        if request.exists() {
            let _ = std::fs::remove_file(&request);
            self.show(ctx);
        }
        if background && self.last_scan.elapsed() >= Duration::from_secs(self.cfg.scan_minutes * 60)
        {
            self.start_scan();
        }
        // Keep ticking even when idle: the show-request file and the scan timer are only seen on a frame.
        if self.scanning() {
            ctx.request_repaint_after(Duration::from_millis(250));
        } else {
            ctx.request_repaint_after(Duration::from_secs(1));
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if self.hidden && !self.has_tray() {
            self.hidden = false;
        }
        if self.hidden {
            // --tray start, or still hidden after closing: keep the window out of sight.
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }
        if ctx.input(|i| i.viewport().close_requested()) && self.has_tray() && !self.quitting {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            self.hidden = true;
            self.message = "Hangar keeps running in the tray".into();
        }
        let scanning = self.scanning();
        egui::Panel::top("top").show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("Hangar", |ui| {
                    let key = ui.ctx().format_shortcut(&SETTINGS_KEY);
                    if ui
                        .add(egui::Button::new("Settings…").shortcut_text(key))
                        .clicked()
                    {
                        self.open_settings();
                    }
                    if ui.button("Check for updates").clicked() {
                        self.open_settings();
                        self.check_updates();
                    }
                    if ui.button("Clean up…").clicked() {
                        self.show_clean = true;
                    }
                    ui.separator();
                    if ui.button("Quit Hangar").clicked() {
                        self.quit(&ctx);
                    }
                });
                ui.menu_button("View", |ui| {
                    ui.menu_button("Theme", |ui| {
                        for (key, label) in THEMES {
                            if ui
                                .radio_value(&mut self.cfg.theme, key.to_string(), label)
                                .changed()
                            {
                                apply_theme(ui.ctx(), key);
                                if let Err(e) = crate::save_config(&self.cfg_path, &self.cfg) {
                                    self.message = format!("theme not saved: {e}");
                                }
                            }
                        }
                    });
                });
            });
            ui.horizontal(|ui| {
                ui.label("Filter");
                ui.text_edit_singleline(&mut self.filter);
                ui.checkbox(&mut self.at_risk_only, "At risk only")
                    .on_hover_text("No git, no remote, uncommitted changes or unpushed commits");
                if ui
                    .add_enabled(!scanning, egui::Button::new("Rescan"))
                    .clicked()
                {
                    self.start_scan();
                }
                if scanning {
                    ui.spinner();
                }
                ui.label(format!("{} projects", self.rows.len()));
                ui.weak(&self.message);
            });
        });

        if self.show_clean {
            let ctx = ui.ctx().clone();
            self.clean_window(&ctx);
        }
        if self.settings.is_some() {
            let ctx = ui.ctx().clone();
            self.settings_window(&ctx);
        }

        let (action, mut clicked) = self.detail_panel(ui, scanning);
        let mut toggle: Option<String> = None;
        if let Some(a) = action {
            self.apply(a);
        }

        egui::CentralPanel::default().show(ui, |ui| {
            let f = self.filter.to_lowercase();
            egui::ScrollArea::both().show(ui, |ui| {
                egui::Grid::new("projects")
                    .striped(true)
                    .num_columns(13)
                    .show(ui, |ui| {
                        for (h, tip) in COLUMNS {
                            ui.strong(format!("{h} ℹ")).on_hover_text(tip);
                        }
                        ui.end_row();
                        let haystack = |r: &Row| {
                            format!(
                                "{} {} {} {} {} {}",
                                r.name, r.path, r.tags, r.shown_status, r.stack, r.description
                            )
                            .to_lowercase()
                        };
                        for r in self
                            .rows
                            .iter()
                            .filter(|r| !self.at_risk_only || r.at_risk)
                            .filter(|r| f.is_empty() || haystack(r).contains(&f))
                        {
                            let is_sel = self.selected.as_ref().is_some_and(|s| s.path == r.path);
                            let name = if r.missing {
                                format!("{} (missing)", r.name)
                            } else {
                                r.name.clone()
                            };
                            let pads = self.scratch.get(&r.path);
                            let open_now = self.expanded.contains(&r.path);
                            ui.horizontal(|ui| {
                                match pads {
                                    Some(p) => {
                                        let arrow = if open_now { "▾" } else { "▸" };
                                        if ui
                                            .small_button(arrow)
                                            .on_hover_text(format!(
                                                "{} Claude scratchpads",
                                                p.len()
                                            ))
                                            .clicked()
                                        {
                                            toggle = Some(r.path.clone());
                                        }
                                    }
                                    None => ui.add_space(ui.spacing().interact_size.y + 2.0),
                                }
                                if ui
                                    .selectable_label(is_sel, name)
                                    .on_hover_text(&r.path)
                                    .clicked()
                                {
                                    clicked = Some(r.clone());
                                }
                            });
                            ui.label(
                                Path::new(&r.path)
                                    .parent()
                                    .map_or(String::new(), |p| p.display().to_string()),
                            );
                            // Derived statuses are dimmed so hand-set ones stand out.
                            if r.status.is_empty() {
                                ui.weak(&r.shown_status);
                            } else {
                                ui.label(&r.status);
                            }
                            if r.at_risk {
                                ui.colored_label(ui.visuals().warn_fg_color, scan::git_label(r));
                            } else {
                                let label = scan::git_label(r);
                                let cell = ui.label(&label);
                                if label == "?" {
                                    cell.on_hover_text(UNKNOWN_GIT);
                                }
                            }
                            ui.label(&r.stack);
                            ui.label(if r.clones > 0 {
                                r.clones.to_string()
                            } else {
                                String::new()
                            });
                            ui.label(r.disk_bytes.map_or(String::new(), disk_size));
                            ui.label(&r.tags);
                            ui.label(&r.tools);
                            ui.label(r.sessions.to_string());
                            ui.label(hours(r.active_secs));
                            ui.label(hours(r.week_secs));
                            ui.label(r.last_active.get(..16).unwrap_or("").replace('T', " "));
                            ui.label(r.description.chars().take(120).collect::<String>());
                            ui.end_row();
                            for p in pads.filter(|_| open_now).into_iter().flatten() {
                                let short = p.session.get(..8).unwrap_or(&p.session);
                                let path = p.path.display().to_string();
                                if ui
                                    .link(format!("      ↳ scratchpad {short}"))
                                    .on_hover_text(format!("Open {path}"))
                                    .clicked()
                                {
                                    open(&path);
                                }
                                ui.weak(
                                    p.path
                                        .parent()
                                        .map_or(String::new(), |d| d.display().to_string()),
                                );
                                ui.weak("temp");
                                for _ in 0..8 {
                                    ui.label("");
                                }
                                ui.weak(&p.modified);
                                ui.weak(format!("{} items", p.items));
                                ui.end_row();
                            }
                        }
                    });
            });
        });
        if let Some(r) = clicked {
            self.select(r);
        }
        if let Some(p) = toggle
            && !self.expanded.remove(&p)
        {
            self.expanded.insert(p);
        }
    }
}

/// Editable list of strings with remove buttons and an add button; `dirs` flags paths that aren't folders.
fn edit_list(ui: &mut egui::Ui, list: &mut Vec<String>, dirs: bool) {
    let mut remove = None;
    for (i, item) in list.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(item).desired_width(400.0));
            if ui.button("Remove").clicked() {
                remove = Some(i);
            }
            if dirs && !item.trim().is_empty() && !Path::new(item.trim()).is_dir() {
                ui.colored_label(ui.visuals().warn_fg_color, "folder not found");
            }
        });
    }
    if let Some(i) = remove {
        list.remove(i);
    }
    if ui.button("Add").clicked() {
        list.push(String::new());
    }
}

/// Tray icon with the version, Open / Rescan / Quit, and the macOS app menu; Windows and macOS only
/// (Linux would need GTK).
#[cfg(any(windows, target_os = "macos"))]
mod tray {
    use eframe::egui;
    use std::sync::mpsc::Sender;
    use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
    use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

    pub enum Cmd {
        Show,
        Rescan,
        Quit,
        #[cfg(target_os = "macos")]
        Settings,
    }

    /// One handler for every native menu: muda keeps only the last one set. Handlers run outside
    /// egui; they queue a command and wake the (possibly hidden) window.
    pub fn listen(ctx: &egui::Context, tx: Sender<Cmd>) {
        let ctx = ctx.clone();
        MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
            let cmd = match e.id.as_ref() {
                "open" => Cmd::Show,
                "rescan" => Cmd::Rescan,
                "quit" => Cmd::Quit,
                #[cfg(target_os = "macos")]
                "settings" => Cmd::Settings,
                _ => return,
            };
            let _ = tx.send(cmd);
            ctx.request_repaint();
        }));
    }

    pub fn create(ctx: &egui::Context, tx: Sender<Cmd>) -> Option<TrayIcon> {
        let menu = Menu::new();
        let version = MenuItem::new(format!("Hangar {}", super::VERSION), false, None);
        menu.append_items(&[
            &version,
            &PredefinedMenuItem::separator(),
            &MenuItem::with_id("open", "Open Hangar", true, None),
            &MenuItem::with_id("rescan", "Rescan now", true, None),
            &MenuItem::with_id("quit", "Quit Hangar", true, None),
        ])
        .ok()?;
        let png = eframe::icon_data::from_png_bytes(include_bytes!("../assets/hangar.png")).ok()?;
        let icon = tray_icon::Icon::from_rgba(png.rgba, png.width, png.height).ok()?;
        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("Hangar")
            .with_icon(icon)
            .build()
            .ok()?;
        let ctx = ctx.clone();
        TrayIconEvent::set_event_handler(Some(move |e: TrayIconEvent| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = e
            {
                let _ = tx.send(Cmd::Show);
                ctx.request_repaint();
            }
        }));
        Some(tray)
    }

    /// Replaces winit's default app menu, adding Settings… ⌘, where Mac users look for it.
    #[cfg(target_os = "macos")]
    pub fn app_menu() {
        use tray_icon::menu::accelerator::{Accelerator, Code, Modifiers};
        use tray_icon::menu::{AboutMetadata, Submenu};
        let key = |code| Some(Accelerator::new(Modifiers::META, code));
        let about = AboutMetadata {
            version: Some(super::VERSION.into()),
            ..Default::default()
        };
        let app = Submenu::new("Hangar", true);
        let window = Submenu::new("Window", true);
        let menu = Menu::new();
        let built = app
            .append_items(&[
                &PredefinedMenuItem::about(Some("About Hangar"), Some(about)),
                &PredefinedMenuItem::separator(),
                &MenuItem::with_id("settings", "Settings…", true, key(Code::Comma)),
                &PredefinedMenuItem::separator(),
                &PredefinedMenuItem::services(None),
                &PredefinedMenuItem::separator(),
                &PredefinedMenuItem::hide(None),
                &PredefinedMenuItem::hide_others(None),
                &PredefinedMenuItem::show_all(None),
                &PredefinedMenuItem::separator(),
                // Our own Quit, so background mode really quits instead of hiding.
                &MenuItem::with_id("quit", "Quit Hangar", true, key(Code::KeyQ)),
            ])
            .and_then(|()| {
                window.append_items(&[
                    &PredefinedMenuItem::minimize(None),
                    &PredefinedMenuItem::fullscreen(None),
                ])
            })
            .and_then(|()| menu.append_items(&[&app, &window]));
        if built.is_ok() {
            menu.init_for_nsapp();
            // NSApp keeps the menu for the life of the app; muda must not tear down its items.
            std::mem::forget(menu);
        }
    }
}

fn apply_theme(ctx: &egui::Context, theme: &str) {
    use egui::{Color32, Theme, ThemePreference, Visuals};
    ctx.set_visuals_of(Theme::Dark, Visuals::dark());
    ctx.set_visuals_of(Theme::Light, Visuals::light());
    match theme {
        "dark" => ctx.set_theme(ThemePreference::Dark),
        "light" => ctx.set_theme(ThemePreference::Light),
        "hangar" => {
            // Colours from the logo: navy hangar, coral and cyan accents.
            let coral = Color32::from_rgb(0xE8, 0x7A, 0x5D);
            let mut v = Visuals::dark();
            v.panel_fill = Color32::from_rgb(0x14, 0x1B, 0x26);
            v.window_fill = Color32::from_rgb(0x18, 0x21, 0x31);
            v.extreme_bg_color = Color32::from_rgb(0x0E, 0x14, 0x1D);
            v.faint_bg_color = Color32::from_rgb(0x1A, 0x25, 0x36);
            v.hyperlink_color = Color32::from_rgb(0x4F, 0xC3, 0xF7);
            v.selection.bg_fill = coral.linear_multiply(0.6);
            v.selection.stroke.color = Color32::WHITE;
            v.widgets.hovered.bg_stroke.color = Color32::from_rgb(0x4F, 0xC3, 0xF7);
            v.widgets.active.bg_fill = coral;
            ctx.set_visuals_of(Theme::Dark, v);
            ctx.set_theme(ThemePreference::Dark);
        }
        _ => ctx.set_theme(ThemePreference::System),
    }
}

/// "812 MB", "1.4 GB": decimal units, one decimal from GB up.
pub fn disk_size(bytes: i64) -> String {
    match bytes as f64 {
        b if b >= 1e9 => format!("{:.1} GB", b / 1e9),
        b if b >= 1e6 => format!("{:.0} MB", b / 1e6),
        b => format!("{:.0} KB", b / 1e3),
    }
}

pub fn hours(secs: f64) -> String {
    format!("{:.1}h", secs / 3600.0)
}

const VERSION: &str = env!("CARGO_PKG_VERSION");
/// ⌘, on macOS, Ctrl+, elsewhere.
const SETTINGS_KEY: egui::KeyboardShortcut =
    egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::Comma);
const RELEASES_URL: &str = "https://github.com/evandervecht/hangar/releases/latest";
/// HTTPS only, redirects included, and no more than a release's JSON.
const CURL_ARGS: &[&str] = &[
    "-fsSL",
    "--max-time",
    "10",
    "--proto",
    "=https",
    "--proto-redir",
    "=https",
    "--max-filesize",
    "1M",
];

/// Version of the latest GitHub release. Uses the curl that ships with Windows 10+ and macOS, so
/// Hangar needs no HTTP library; it only runs when the user clicks "Check for updates".
fn latest_release() -> Result<String, String> {
    let out = scan::quiet_cmd("curl")
        .args(CURL_ARGS)
        .arg("https://api.github.com/repos/evandervecht/hangar/releases/latest")
        .output()
        .map_err(|e| format!("curl: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())?;
    v["tag_name"]
        .as_str()
        .map(|t| t.trim_start_matches('v').to_string())
        .ok_or_else(|| "no release found".into())
}

/// `latest` is a higher a.b.c than `current`; a part that isn't a number counts as 0.
fn newer(latest: &str, current: &str) -> bool {
    let parts = |s: &str| -> Vec<u64> { s.split('.').map(|p| p.parse().unwrap_or(0)).collect() };
    parts(latest) > parts(current)
}

fn open(path: &str) {
    let cmd = if cfg!(windows) {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    let _ = std::process::Command::new(cmd).arg(path).spawn();
}

/// Reopen a session in its own tool, in a new terminal window in the folder it started in.
fn resume(s: &scan::Resumable) -> Result<(), String> {
    // The id comes from a log file and goes through a shell: only ids made of id characters,
    // and never one the tool would read as a flag.
    if s.session.is_empty()
        || s.session.starts_with('-')
        || !s
            .session
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        return Err(format!("unexpected session id {:?}", s.session));
    }
    let cmd = match s.tool.as_str() {
        "claude" => format!("claude --resume {}", s.session),
        "codex" => format!("codex resume {}", s.session),
        t => return Err(format!("{t} sessions can't be resumed")),
    };
    #[cfg(windows)]
    let mut c = {
        use std::os::windows::process::CommandExt;
        // start's first quoted argument is the window title.
        let mut c = scan::quiet_cmd("cmd");
        c.raw_arg(format!("/C start \"Hangar\" cmd /K {cmd}"));
        c
    };
    #[cfg(target_os = "macos")]
    let mut c = {
        let mut c = std::process::Command::new("osascript");
        c.args(["-e", &terminal_script(&s.cwd, &cmd)]);
        c
    };
    #[cfg(not(any(windows, target_os = "macos")))]
    let result = Err(format!("run `{cmd}` in {}", s.cwd));
    #[cfg(any(windows, target_os = "macos"))]
    let result = c
        .current_dir(&s.cwd)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string());
    result
}

/// AppleScript that runs `cmd` in `cwd` in a new Terminal window. `cwd` comes from a log file:
/// shell-quoted, then the whole line escaped for an AppleScript string.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn terminal_script(cwd: &str, cmd: &str) -> String {
    let sh = format!("cd '{}' && {cmd}", cwd.replace('\'', r"'\''"));
    let line = sh.replace('\\', r"\\").replace('"', r#"\""#);
    format!("tell application \"Terminal\"\nactivate\ndo script \"{line}\"\nend tell")
}

#[test]
fn terminal_script_quotes_odd_folder_names() {
    assert_eq!(
        terminal_script(r#"/Users/me/it's "a\b""#, "claude --resume x"),
        "tell application \"Terminal\"\nactivate\ndo script \
         \"cd '/Users/me/it'\\\\''s \\\"a\\\\b\\\"' && claude --resume x\"\nend tell"
    );
}

/// `git@host:a/b.git` and `https://host/a/b.git` -> `https://host/a/b`; other remotes (file paths) get no button.
fn browser_url(origin: &str) -> Option<String> {
    let url = if let Some(rest) = origin.strip_prefix("git@") {
        format!("https://{}", rest.replacen(':', "/", 1))
    } else if origin.starts_with("https://") || origin.starts_with("http://") {
        origin.to_string()
    } else {
        return None;
    };
    Some(url.trim_end_matches(".git").to_string())
}

#[test]
fn browser_urls() {
    assert_eq!(
        browser_url("git@github.com:a/b.git").as_deref(),
        Some("https://github.com/a/b")
    );
    assert_eq!(
        browser_url("https://github.com/a/b").as_deref(),
        Some("https://github.com/a/b")
    );
    assert_eq!(browser_url("local only"), None);
}

#[test]
fn resume_refuses_ids_a_shell_would_read() {
    let s = |session: &str| scan::Resumable {
        tool: "claude".into(),
        session: session.into(),
        last: String::new(),
        cwd: ".".into(),
    };
    for bad in ["", "a&calc", "a b", "a\"b", "a;rm", "-x", "--help"] {
        assert!(
            resume(&s(bad))
                .unwrap_err()
                .starts_with("unexpected session id")
        );
    }
}

#[test]
fn update_check_stays_on_https_and_small() {
    let args = CURL_ARGS.join(" ");
    for want in [
        "--proto =https",
        "--proto-redir =https",
        "--max-filesize 1M",
    ] {
        assert!(args.contains(want), "{want} missing from {args}");
    }
}

#[test]
fn newer_compares_numbers_not_text() {
    assert!(newer("0.7.10", "0.7.9"));
    assert!(newer("1.0.0", "0.9.9"));
    assert!(!newer("0.7.1", "0.7.1"));
    assert!(!newer("0.7.0", "0.7.1"));
}
