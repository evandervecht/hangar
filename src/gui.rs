use crate::Config;
use crate::scan::{self, Row};
use eframe::egui;
use rusqlite::Connection;
use std::path::{Path, PathBuf};

const STATUSES: [&str; 3] = ["active", "paused", "archived"];

struct App {
    db: Connection,
    home: PathBuf,
    cfg: Config,
    rows: Vec<Row>,
    filter: String,
    selected: Option<Row>,
    full_text: String,
    message: String,
}

pub fn run(db: Connection, home: PathBuf, cfg: Config, message: String) -> eframe::Result {
    let rows = scan::list(&db, &cfg.excludes).unwrap_or_default();
    let app = App {
        db,
        home,
        cfg,
        rows,
        filter: String::new(),
        selected: None,
        full_text: String::new(),
        message,
    };
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 800.0])
            .with_icon(
                eframe::icon_data::from_png_bytes(include_bytes!("../assets/hangar.png"))
                    .expect("bundled icon is a valid png"),
            ),
        ..Default::default()
    };
    eframe::run_native("Hangar", opts, Box::new(|_| Ok(Box::new(app))))
}

impl App {
    // Rows are loaded on launch/rescan/save only; egui repaints often and the list query isn't free.
    fn reload(&mut self) {
        match scan::list(&self.db, &self.cfg.excludes) {
            Ok(rows) => self.rows = rows,
            Err(e) => self.message = format!("load failed: {e}"),
        }
    }

    fn select(&mut self, row: Row) {
        self.full_text = scan::description_file(Path::new(&row.path))
            .and_then(|f| std::fs::read_to_string(f).ok())
            .unwrap_or_else(|| "(no README.md, CLAUDE.md or AGENTS.md)".into());
        self.selected = Some(row);
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::top("top").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Filter");
                ui.text_edit_singleline(&mut self.filter);
                // shortcut: rescan blocks the UI for a few seconds, move to a thread with the v2 background service.
                if ui.button("Rescan").clicked() {
                    self.message = match scan::run(
                        &mut self.db,
                        &self.home,
                        &self.cfg.roots,
                        &self.cfg.excludes,
                    ) {
                        Ok(s) => format!("imported {} log files, {} projects", s.files, s.projects),
                        Err(e) => format!("scan failed: {e}"),
                    };
                    self.reload();
                }
                ui.label(format!("{} projects", self.rows.len()));
                ui.weak(&self.message);
            });
        });

        if let Some(sel) = self.selected.as_mut() {
            let mut save = false;
            egui::Panel::right("detail")
                .resizable(true)
                .default_size(450.0)
                .show(ui, |ui| {
                    ui.heading(&sel.name);
                    ui.label(&sel.path);
                    ui.label(&sel.origin);
                    ui.label(format!(
                        "{} sessions ({}), first seen {}",
                        sel.sessions,
                        sel.tools,
                        sel.first_seen.get(..10).unwrap_or("")
                    ));
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
                        egui::ComboBox::from_id_salt("status")
                            .selected_text(sel.status.clone())
                            .show_ui(ui, |ui| {
                                for s in STATUSES {
                                    ui.selectable_value(&mut sel.status, s.to_string(), s);
                                }
                            });
                        ui.end_row();
                        ui.label("Notes");
                        ui.text_edit_multiline(&mut sel.notes);
                        ui.end_row();
                    });
                    save = ui.button("Save").clicked();
                    ui.separator();
                    egui::ScrollArea::vertical().show(ui, |ui| ui.monospace(&self.full_text));
                });
            if save {
                if let Err(e) = scan::save_manual(&self.db, sel) {
                    self.message = format!("save failed: {e}");
                }
                self.reload();
            }
        }

        let mut clicked = None;
        egui::CentralPanel::default().show(ui, |ui| {
            let f = self.filter.to_lowercase();
            egui::ScrollArea::both().show(ui, |ui| {
                egui::Grid::new("projects")
                    .striped(true)
                    .num_columns(10)
                    .show(ui, |ui| {
                        for h in [
                            "Project",
                            "Location",
                            "Status",
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
                                "{} {} {} {} {}",
                                r.name, r.path, r.tags, r.status, r.description
                            )
                            .to_lowercase()
                        };
                        for r in self
                            .rows
                            .iter()
                            .filter(|r| f.is_empty() || haystack(r).contains(&f))
                        {
                            let is_sel = self.selected.as_ref().is_some_and(|s| s.path == r.path);
                            let name = if r.missing {
                                format!("{} (missing)", r.name)
                            } else {
                                r.name.clone()
                            };
                            if ui
                                .selectable_label(is_sel, name)
                                .on_hover_text(&r.path)
                                .clicked()
                            {
                                clicked = Some(r.clone());
                            }
                            ui.label(
                                Path::new(&r.path)
                                    .parent()
                                    .map_or(String::new(), |p| p.display().to_string()),
                            );
                            ui.label(&r.status);
                            ui.label(&r.tags);
                            ui.label(&r.tools);
                            ui.label(r.sessions.to_string());
                            ui.label(hours(r.active_secs));
                            ui.label(hours(r.week_secs));
                            ui.label(r.last_active.get(..16).unwrap_or("").replace('T', " "));
                            ui.label(r.description.chars().take(120).collect::<String>());
                            ui.end_row();
                        }
                    });
            });
        });
        if let Some(r) = clicked {
            self.select(r);
        }
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
