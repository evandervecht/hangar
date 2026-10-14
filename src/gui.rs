use crate::Config;
use crate::scan::{self, Row, Scratchpad};
use eframe::egui;
use rusqlite::Connection;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

const THEMES: [(&str, &str); 4] = [
    ("", "System"),
    ("dark", "Dark"),
    ("light", "Light"),
    ("hangar", "Hangar"),
];

const UNKNOWN_GIT: &str = "Git couldn't read this repo, often 'dubious ownership' (the folder belongs to another user). Run `git status` in it to see why.";

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
    /// Started with --tray: background mode for this session even if the setting is off.
    tray_start: bool,
    hidden: bool,
    quitting: bool,
    #[cfg(any(windows, target_os = "macos"))]
    tray: Option<tray::Tray>,
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
        tray_start,
        hidden: tray_start,
        quitting: false,
        #[cfg(any(windows, target_os = "macos"))]
        tray: None,
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
    }

    fn select(&mut self, row: Row) {
        self.full_text = scan::description_file(Path::new(&row.path))
            .and_then(|f| std::fs::read_to_string(f).ok())
            .unwrap_or_else(|| "(no README.md, CLAUDE.md or AGENTS.md)".into());
        self.confirm_name.clear();
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
            Action::TrashFolder => match trash::delete(&sel.path) {
                Ok(()) => {
                    self.selected = None;
                    let _ = self.db.execute(
                        "UPDATE projects SET missing = 1 WHERE path = ?1",
                        [&sel.path],
                    );
                    format!("moved {} to the Recycle Bin / Trash", sel.path)
                }
                Err(e) => format!("delete failed: {e}"),
            },
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
            scan::run(&mut db, &home, &roots, &excludes).map_err(|e| e.to_string())
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

    fn settings_window(&mut self, ctx: &egui::Context) {
        let scanning = self.scanning();
        let Some(draft) = self.settings.as_mut() else {
            return;
        };
        let (mut save, mut cancel) = (false, false);
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
            });
        if save {
            let mut cfg = self.settings.take().expect("settings open");
            for list in [&mut cfg.roots, &mut cfg.excludes] {
                list.iter_mut().for_each(|s| *s = s.trim().to_string());
                list.retain(|s| !s.is_empty());
            }
            // Starting at login only makes sense in the tray.
            cfg.background |= cfg.autostart;
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
                        self.old_logs = Some(scan::old_logs(&self.home, self.log_days));
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
                        self.message = match scan::trash_logs(files) {
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

impl eframe::App for App {
    // Runs before every frame and, while the window is hidden, on each requested repaint.
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_scan();
        let background = self.background();
        #[cfg(any(windows, target_os = "macos"))]
        {
            if background && self.tray.is_none() && !self.tray_failed {
                self.tray = tray::create(ctx);
                self.tray_failed = self.tray.is_none();
            } else if !background {
                self.tray = None;
                self.tray_failed = false;
            }
            let cmds: Vec<tray::Cmd> = self
                .tray
                .as_ref()
                .map_or(vec![], |t| t.rx.try_iter().collect());
            for cmd in cmds {
                match cmd {
                    tray::Cmd::Show => self.show(ctx),
                    tray::Cmd::Rescan => self.start_scan(),
                    tray::Cmd::Quit => {
                        self.quitting = true;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
            }
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
        if self.scanning() {
            ctx.request_repaint_after(Duration::from_millis(250));
        } else if background {
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
                if ui.button("Clean up").clicked() {
                    self.show_clean = !self.show_clean;
                }
                if ui.button("Settings").clicked() {
                    self.settings = Some(self.cfg.clone());
                }
                let current = THEMES
                    .iter()
                    .find(|(k, _)| *k == self.cfg.theme)
                    .map_or("System", |(_, l)| l);
                egui::ComboBox::from_id_salt("theme")
                    .selected_text(format!("Theme: {current}"))
                    .show_ui(ui, |ui| {
                        for (key, label) in THEMES {
                            if ui
                                .selectable_value(&mut self.cfg.theme, key.to_string(), label)
                                .changed()
                            {
                                apply_theme(ui.ctx(), key);
                                if let Err(e) = crate::save_config(&self.cfg_path, &self.cfg) {
                                    self.message = format!("theme not saved: {e}");
                                }
                            }
                        }
                    });
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

        let mut action = None;
        let mut clicked = None;
        let mut toggle: Option<String> = None;
        if let Some(sel) = self.selected.as_mut() {
            let guard = scan::delete_guard(&sel.path, &self.home, &self.cfg.roots);
            let key = scan::origin_key(&sel.origin);
            let clones: Vec<&Row> = self
                .rows
                .iter()
                .filter(|r| {
                    r.path != sel.path && key.is_some() && scan::origin_key(&r.origin) == key
                })
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
                                if sel.git_changed.is_some_and(|n| n > 0)
                                    || sel.git_unpushed.is_some_and(|n| n > 0)
                                    || sel.origin == "local only"
                                {
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
        }
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
                        for h in [
                            "Project",
                            "Location",
                            "Status",
                            "Git",
                            "Stack",
                            "Clones",
                            "Tags",
                            "Tools",
                            "Sessions",
                            "Active",
                            "7 days",
                            "Last active (UTC)",
                            "Description",
                        ] {
                            ui.strong(h);
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

/// Tray icon with Open / Rescan / Quit; Windows and macOS only (Linux would need GTK).
#[cfg(any(windows, target_os = "macos"))]
mod tray {
    use eframe::egui;
    use std::sync::mpsc::{Receiver, channel};
    use tray_icon::menu::{Menu, MenuEvent, MenuItem};
    use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

    pub enum Cmd {
        Show,
        Rescan,
        Quit,
    }

    pub struct Tray {
        _icon: TrayIcon,
        pub rx: Receiver<Cmd>,
    }

    pub fn create(ctx: &egui::Context) -> Option<Tray> {
        let menu = Menu::new();
        let open = MenuItem::new("Open Hangar", true, None);
        let rescan = MenuItem::new("Rescan now", true, None);
        let quit = MenuItem::new("Quit Hangar", true, None);
        menu.append_items(&[&open, &rescan, &quit]).ok()?;
        let png = eframe::icon_data::from_png_bytes(include_bytes!("../assets/hangar.png")).ok()?;
        let icon = tray_icon::Icon::from_rgba(png.rgba, png.width, png.height).ok()?;
        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("Hangar")
            .with_icon(icon)
            .build()
            .ok()?;
        let (tx, rx) = channel();
        // Handlers run outside egui; they queue a command and wake the (possibly hidden) window.
        let (ids, menu_tx, menu_ctx) = (
            (open.id().clone(), rescan.id().clone(), quit.id().clone()),
            tx.clone(),
            ctx.clone(),
        );
        MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
            let cmd = if e.id == ids.0 {
                Cmd::Show
            } else if e.id == ids.1 {
                Cmd::Rescan
            } else if e.id == ids.2 {
                Cmd::Quit
            } else {
                return;
            };
            let _ = menu_tx.send(cmd);
            menu_ctx.request_repaint();
        }));
        let icon_ctx = ctx.clone();
        TrayIconEvent::set_event_handler(Some(move |e: TrayIconEvent| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = e
            {
                let _ = tx.send(Cmd::Show);
                icon_ctx.request_repaint();
            }
        }));
        Some(Tray { _icon: tray, rx })
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

pub fn hours(secs: f64) -> String {
    format!("{:.1}h", secs / 3600.0)
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
