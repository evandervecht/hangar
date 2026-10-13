use rusqlite::{Connection, params};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

/// Gaps between log lines longer than this don't count as active time.
pub const IDLE_SECS: f64 = 15.0 * 60.0;
const SKIP_DIRS: [&str; 3] = ["node_modules", "target", ".venv"];
const ROOT_DEPTH: usize = 4;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS projects(
  path TEXT PRIMARY KEY, name TEXT NOT NULL, origin TEXT NOT NULL, description TEXT NOT NULL,
  missing INTEGER NOT NULL, host TEXT NOT NULL, first_seen TEXT NOT NULL,
  tags TEXT NOT NULL DEFAULT '', status TEXT NOT NULL DEFAULT '', notes TEXT NOT NULL DEFAULT '');
CREATE TABLE IF NOT EXISTS events(file TEXT NOT NULL, project TEXT NOT NULL, tool TEXT NOT NULL,
  session TEXT NOT NULL, ts TEXT NOT NULL);
CREATE INDEX IF NOT EXISTS events_file ON events(file);
CREATE INDEX IF NOT EXISTS events_session ON events(tool, session, ts);
CREATE TABLE IF NOT EXISTS files(path TEXT PRIMARY KEY, mtime INTEGER NOT NULL, size INTEGER NOT NULL);
";

pub fn open(path: &Path) -> rusqlite::Result<Connection> {
    let db = Connection::open(path)?;
    // The window reads while a background scan writes; WAL lets both proceed, the timeout covers commits.
    db.busy_timeout(std::time::Duration::from_secs(60))?;
    db.pragma_update(None, "journal_mode", "WAL")?;
    db.execute_batch(SCHEMA)?;
    migrate(&db)?;
    Ok(db)
}

fn migrate(db: &Connection) -> rusqlite::Result<()> {
    let version = db.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))?;
    if version < 1 {
        // v0.1.0 stored 'active' as a default, not a choice; empty status now means "derived from activity".
        db.execute_batch(
            "UPDATE projects SET status = '' WHERE status = 'active'; PRAGMA user_version = 1;",
        )?;
    }
    if version < 2 {
        // Nullable/defaulted columns, so older Hangar versions still read this database.
        db.execute_batch(
            "BEGIN;
             ALTER TABLE projects ADD COLUMN git_changed INTEGER;
             ALTER TABLE projects ADD COLUMN git_unpushed INTEGER;
             ALTER TABLE projects ADD COLUMN git_last_commit TEXT;
             ALTER TABLE projects ADD COLUMN stack TEXT NOT NULL DEFAULT '';
             CREATE TABLE IF NOT EXISTS moves(from_path TEXT PRIMARY KEY, to_path TEXT NOT NULL);
             PRAGMA user_version = 2;
             COMMIT;",
        )?;
    }
    Ok(())
}

pub struct Stats {
    pub files: usize,
    pub projects: usize,
}

/// Import new/changed logs, scan roots, refresh automatic project fields.
pub fn run(
    db: &mut Connection,
    home: &Path,
    roots: &[String],
    excludes: &[String],
) -> rusqlite::Result<Stats> {
    let tx = db.transaction()?;
    let moves: Vec<(String, String)> = {
        let mut st = tx.prepare("SELECT from_path, to_path FROM moves")?;
        st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<_, _>>()?
    };
    let mut cache = HashMap::new();
    let mut files = 0;
    for (tool, file) in log_files(home) {
        let Ok(meta) = fs::metadata(&file) else {
            continue;
        };
        let stamp = |m: &fs::Metadata| {
            m.modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_secs() as i64)
        };
        let (mut mtime, mut size) = (stamp(&meta), meta.len() as i64);
        // SQLite stores (Hermes) write to a -wal file first; count it, or new sessions go unseen until a checkpoint.
        if let Ok(wal) = fs::metadata(format!("{}-wal", file.display())) {
            mtime = mtime.max(stamp(&wal));
            size += wal.len() as i64;
        }
        let key = file.to_string_lossy().to_string();
        let seen: Option<(i64, i64)> = tx
            .query_row(
                "SELECT mtime, size FROM files WHERE path = ?1",
                [&key],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .ok();
        if seen == Some((mtime, size)) {
            continue;
        }
        tx.execute("DELETE FROM events WHERE file = ?1", [&key])?;
        if tool == "hermes" {
            import_hermes(&tx, &file, &key, &moves, &mut cache)?;
        } else {
            let cwd = if tool == "gemini" {
                gemini_cwd(&file, home)
            } else {
                None
            };
            import_file(&tx, &file, &key, tool, cwd, &moves, &mut cache)?;
        }
        tx.execute(
            "INSERT OR REPLACE INTO files(path, mtime, size) VALUES (?1, ?2, ?3)",
            params![key, mtime, size],
        )?;
        files += 1;
    }

    let mut projects: Vec<String> = {
        let mut st = tx.prepare("SELECT DISTINCT project FROM events")?;
        st.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?
    };
    for root in roots {
        scan_root(Path::new(root), ROOT_DEPTH, excludes, &mut projects);
    }
    projects.sort();
    projects.dedup();
    let host = hostname();
    for p in &projects {
        let dir = Path::new(p);
        let name = dir
            .file_name()
            .map_or(p.clone(), |n| n.to_string_lossy().to_string());
        if !dir.exists() {
            // Gone: keep the last-known origin, description and git facts; they identify a moved project.
            tx.execute(
                "INSERT INTO projects(path, name, origin, description, missing, host, first_seen)
                 VALUES (?1, ?2, 'no git', '', 1, ?3, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
                 ON CONFLICT(path) DO UPDATE SET missing = 1",
                params![p, name, host],
            )?;
            continue;
        }
        let summary = description_file(dir)
            .map(|f| first_paragraph(&fs::read_to_string(f).unwrap_or_default()));
        // Upsert touches automatic columns only: tags/status/notes are the user's.
        tx.execute(
            "INSERT INTO projects(path, name, origin, description, missing, host, first_seen, stack)
             VALUES (?1, ?2, ?3, ?4, 0, ?5, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), ?6)
             ON CONFLICT(path) DO UPDATE SET name = ?2, origin = ?3, description = ?4, missing = 0, host = ?5, stack = ?6",
            params![p, name, origin(dir), summary.unwrap_or_default(), host, stack(dir)],
        )?;
    }
    let repos: Vec<&String> = projects
        .iter()
        .filter(|p| Path::new(p).join(".git").exists() && !excluded(p, excludes))
        .collect();
    for (p, (changed, unpushed, last)) in repos.iter().zip(git_health_all(&repos)) {
        tx.execute(
            "UPDATE projects SET git_changed = ?2, git_unpushed = ?3, git_last_commit = ?4 WHERE path = ?1",
            params![p, changed, unpushed, last],
        )?;
    }
    // Projects not seen this run (log deleted, repo removed) keep their history but get re-checked.
    let mut st = tx.prepare("SELECT path FROM projects")?;
    let all: Vec<String> = st.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?;
    drop(st);
    for p in all {
        tx.execute(
            "UPDATE projects SET missing = ?2 WHERE path = ?1",
            params![p, !Path::new(&p).exists()],
        )?;
    }
    tx.commit()?;
    let visible = projects.iter().filter(|p| !excluded(p, excludes)).count();
    Ok(Stats {
        files,
        projects: visible,
    })
}

fn import_file(
    tx: &Connection,
    file: &Path,
    key: &str,
    tool: &str,
    gemini_cwd: Option<String>,
    moves: &[(String, String)],
    cache: &mut HashMap<String, String>,
) -> rusqlite::Result<()> {
    let Ok(f) = fs::File::open(file) else {
        return Ok(());
    };
    if tool == "gemini" && gemini_cwd.is_none() {
        return Ok(()); // project folder unknown: nothing to attribute the session to
    }
    let mut codex = Codex::default();
    let mut gemini = Gemini {
        cwd: gemini_cwd.unwrap_or_default(),
        ..Default::default()
    };
    let mut ins = tx.prepare_cached(
        "INSERT INTO events(file, project, tool, session, ts) VALUES (?1, ?2, ?3, ?4, ?5)",
    )?;
    for line in BufReader::new(f).lines() {
        // Unparseable lines (partial writes, unknown shapes) are skipped.
        let Ok(line) = line else { continue };
        let Ok(v) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let ev = match tool {
            "claude" => claude_event(&v),
            "gemini" => gemini.feed(&v),
            _ => codex.feed(&v),
        };
        let Some((cwd, session, ts)) = ev else {
            continue;
        };
        let project = cache
            .entry(cwd.clone())
            .or_insert_with(|| project_root(&apply_moves(&normalize(&cwd), moves)));
        ins.execute(params![key, project.as_str(), tool, session, ts])?;
    }
    Ok(())
}

/// Hermes Agent keeps sessions in SQLite (`sessions.cwd`, `messages.timestamp` in epoch seconds).
/// Opened read-only; a locked or unreadable store is skipped, not an error.
fn import_hermes(
    tx: &Connection,
    file: &Path,
    key: &str,
    moves: &[(String, String)],
    cache: &mut HashMap<String, String>,
) -> rusqlite::Result<()> {
    use rusqlite::OpenFlags;
    let Ok(src) = Connection::open_with_flags(
        file,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) else {
        return Ok(());
    };
    let rows: Vec<(String, String, String)> = match src.prepare(
        "SELECT s.id, s.cwd, strftime('%Y-%m-%dT%H:%M:%fZ', m.timestamp, 'unixepoch')
         FROM messages m JOIN sessions s ON s.id = m.session_id
         WHERE s.cwd IS NOT NULL AND s.cwd <> '' AND m.timestamp IS NOT NULL",
    ) {
        Ok(mut st) => st
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .map(|it| it.filter_map(Result::ok).collect())
            .unwrap_or_default(),
        Err(_) => return Ok(()), // schema we don't know: skip rather than fail the scan
    };
    let mut ins = tx.prepare_cached(
        "INSERT INTO events(file, project, tool, session, ts) VALUES (?1, ?2, 'hermes', ?3, ?4)",
    )?;
    for (session, cwd, ts) in rows {
        let project = cache
            .entry(cwd.clone())
            .or_insert_with(|| project_root(&apply_moves(&normalize(&cwd), moves)));
        ins.execute(params![key, project.as_str(), session, ts])?;
    }
    Ok(())
}

/// Gemini CLI sessions: first line holds `sessionId` and `startTime`, message lines a `timestamp`.
#[derive(Default)]
struct Gemini {
    cwd: String,
    session: String,
    subagent: bool,
}

impl Gemini {
    fn feed(&mut self, v: &Value) -> Option<(String, String, String)> {
        if let Some(id) = v["sessionId"].as_str() {
            self.session = id.into();
            // Subagent sessions run inside a main session; like Claude's subagent files they aren't counted.
            self.subagent = v["kind"].as_str() == Some("subagent");
            let start = v["startTime"].as_str()?;
            return (!self.subagent)
                .then(|| (self.cwd.clone(), self.session.clone(), start.into()));
        }
        if self.session.is_empty() || self.subagent {
            return None;
        }
        Some((
            self.cwd.clone(),
            self.session.clone(),
            v["timestamp"].as_str()?.into(),
        ))
    }
}

/// Project folder of a Gemini CLI chat file `~/.gemini/tmp/<key>/chats/session-*.jsonl`:
/// from `<key>/.project_root`, else from `~/.gemini/projects.json` (path -> key).
fn gemini_cwd(file: &Path, home: &Path) -> Option<String> {
    let key_dir = file.parent()?.parent()?;
    if let Ok(root) = fs::read_to_string(key_dir.join(".project_root")) {
        let root = root.trim();
        if !root.is_empty() {
            return Some(root.to_string());
        }
    }
    let key = key_dir.file_name()?.to_str()?;
    let map: Value =
        serde_json::from_str(&fs::read_to_string(home.join(".gemini").join("projects.json")).ok()?)
            .ok()?;
    map["projects"]
        .as_object()?
        .iter()
        .find(|(_, v)| v.as_str() == Some(key))
        .map(|(path, _)| path.clone())
}

/// (cwd, session, timestamp) from one Claude Code transcript line.
fn claude_event(v: &Value) -> Option<(String, String, String)> {
    Some((
        v["cwd"].as_str()?.into(),
        v["sessionId"].as_str()?.into(),
        v["timestamp"].as_str()?.into(),
    ))
}

/// Codex rollouts state cwd/session once (session_meta) and on change (turn_context).
#[derive(Default)]
struct Codex {
    cwd: String,
    session: String,
}

impl Codex {
    fn feed(&mut self, v: &Value) -> Option<(String, String, String)> {
        let p = &v["payload"];
        match v["type"].as_str() {
            Some("session_meta") => {
                if let Some(id) = p["id"].as_str() {
                    self.session = id.into();
                }
                if let Some(c) = p["cwd"].as_str() {
                    self.cwd = c.into();
                }
            }
            Some("turn_context") => {
                if let Some(c) = p["cwd"].as_str() {
                    self.cwd = c.into();
                }
            }
            _ => {}
        }
        if self.cwd.is_empty() || self.session.is_empty() {
            return None;
        }
        Some((
            self.cwd.clone(),
            self.session.clone(),
            v["timestamp"].as_str()?.into(),
        ))
    }
}

/// One spelling per folder, so logs and the root scan agree on project keys.
pub fn normalize(p: &str) -> String {
    if let Ok(c) = fs::canonicalize(p) {
        let s = c.to_string_lossy();
        return match s.strip_prefix(r"\\?\") {
            Some(rest) if !rest.starts_with("UNC\\") => rest.to_string(),
            _ => s.to_string(),
        };
    }
    let mut s = p.trim_end_matches(['\\', '/']).to_string();
    if s.is_empty() {
        return p.to_string();
    }
    if s.as_bytes().get(1) == Some(&b':') {
        s[..1].make_ascii_uppercase();
    }
    s
}

/// Every Claude Code and Codex session log under `home`, tagged with its tool.
fn log_files(home: &Path) -> Vec<(&'static str, PathBuf)> {
    let mut logs = Vec::new();
    for dir in read_dirs(&home.join(".claude").join("projects")) {
        logs.extend(jsonl_in(&dir).into_iter().map(|f| ("claude", f)));
    }
    logs.extend(
        jsonl_tree(&home.join(".codex").join("sessions"))
            .into_iter()
            .map(|f| ("codex", f)),
    );
    for key_dir in read_dirs(&home.join(".gemini").join("tmp")) {
        logs.extend(
            jsonl_in(&key_dir.join("chats"))
                .into_iter()
                .map(|f| ("gemini", f)),
        );
    }
    let hermes_home = std::env::var_os("HERMES_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            if cfg!(windows) {
                std::env::var_os("LOCALAPPDATA")
                    .map_or_else(|| home.join(".hermes"), |d| PathBuf::from(d).join("hermes"))
            } else {
                home.join(".hermes")
            }
        });
    let mut stores = vec![hermes_home.join("state.db")];
    stores.extend(
        read_dirs(&hermes_home.join("profiles"))
            .into_iter()
            .map(|d| d.join("state.db")),
    );
    logs.extend(
        stores
            .into_iter()
            .filter(|f| f.is_file())
            .map(|f| ("hermes", f)),
    );
    logs
}

/// Rewrites a path that lies in (or is) a merged-away project to the project it was merged into.
fn apply_moves(path: &str, moves: &[(String, String)]) -> String {
    for (from, to) in moves {
        if let Ok(rest) = Path::new(path).strip_prefix(from) {
            return if rest.as_os_str().is_empty() {
                to.clone()
            } else {
                Path::new(to).join(rest).to_string_lossy().to_string()
            };
        }
    }
    path.to_string()
}

/// Nearest folder containing `.git`, else the cwd itself.
// shortcut: a deleted repo's sessions fall back to their own cwd (e.g. repo\src), fine while folders rarely vanish.
fn project_root(cwd: &str) -> String {
    let n = normalize(cwd);
    let mut p = Path::new(&n);
    loop {
        if p.join(".git").exists() {
            return p.to_string_lossy().to_string();
        }
        match p.parent() {
            Some(q) => p = q,
            None => return n,
        }
    }
}

/// Exclude patterns match whole path segments, case-insensitive: `Temp` hides `..\Temp\..` but not `templates`.
pub fn excluded(path: &str, excludes: &[String]) -> bool {
    let segs: Vec<String> = path.split(['\\', '/']).map(str::to_lowercase).collect();
    excludes.iter().any(|e| {
        let pat: Vec<String> = e
            .split(['\\', '/'])
            .filter(|s| !s.is_empty())
            .map(str::to_lowercase)
            .collect();
        !pat.is_empty() && segs.windows(pat.len()).any(|w| w == pat.as_slice())
    })
}

fn scan_root(dir: &Path, depth: usize, excludes: &[String], out: &mut Vec<String>) {
    if dir.join(".git").exists() {
        out.push(normalize(&dir.to_string_lossy()));
        return;
    }
    if depth == 0 {
        return;
    }
    let Ok(rd) = fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        // file_type() doesn't follow symlinks, so link loops can't recurse.
        if !e.file_type().is_ok_and(|t| t.is_dir()) {
            continue;
        }
        let p = e.path();
        if SKIP_DIRS.iter().any(|s| e.file_name() == *s) || excluded(&p.to_string_lossy(), excludes)
        {
            continue;
        }
        scan_root(&p, depth - 1, excludes, out);
    }
}

fn read_dirs(dir: &Path) -> Vec<PathBuf> {
    let Ok(rd) = fs::read_dir(dir) else {
        return vec![];
    };
    rd.flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| e.path())
        .collect()
}

fn jsonl_in(dir: &Path) -> Vec<PathBuf> {
    let Ok(rd) = fs::read_dir(dir) else {
        return vec![];
    };
    rd.flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|x| x == "jsonl"))
        .collect()
}

fn jsonl_tree(dir: &Path) -> Vec<PathBuf> {
    let mut out = jsonl_in(dir);
    for d in read_dirs(dir) {
        out.extend(jsonl_tree(&d));
    }
    out
}

pub fn description_file(dir: &Path) -> Option<PathBuf> {
    ["README.md", "CLAUDE.md", "AGENTS.md"]
        .iter()
        .map(|f| dir.join(f))
        .find(|f| f.is_file())
}

/// First prose paragraph of a markdown file: skips frontmatter, headings, badges, html, tables, code.
pub fn first_paragraph(md: &str) -> String {
    let mut out: Vec<&str> = vec![];
    let mut lines = md.lines().peekable();
    if lines.peek().is_some_and(|l| l.trim() == "---") {
        lines.next();
        lines.by_ref().find(|l| l.trim() == "---");
    }
    let mut in_code = false;
    for l in lines {
        let t = l.trim();
        if t.starts_with("```") {
            in_code = !in_code;
        }
        let skip = in_code
            || t.starts_with("```")
            || t.is_empty()
            || ["#", "![", "[!", "<", "|", "---"]
                .iter()
                .any(|p| t.starts_with(p));
        if skip {
            if !out.is_empty() {
                break;
            }
            continue;
        }
        out.push(t);
    }
    plain(&out.join(" "))
}

/// Inline markdown to plain text: `[text](url)` -> `text`, drops `**`, `__` and backticks.
fn plain(s: &str) -> String {
    let s = s.replace("**", "").replace("__", "").replace('`', "");
    let mut out = String::new();
    let mut rest = s.as_str();
    while let Some(open) = rest.find('[') {
        let link = rest[open..]
            .find("](")
            .and_then(|mid| rest[open + mid..].find(')').map(|end| (mid, mid + end)));
        let Some((mid, end)) = link else { break };
        out.push_str(&rest[..open]);
        out.push_str(&rest[open + 1..open + mid]);
        rest = &rest[open + end + 1..];
    }
    out.push_str(rest);
    out
}

pub fn origin(dir: &Path) -> String {
    let dotgit = dir.join(".git");
    let gitdir = if dotgit.is_dir() {
        dotgit
    } else if dotgit.is_file() {
        // Worktrees and submodules: `.git` is a file pointing at the real git dir.
        let s = fs::read_to_string(&dotgit).unwrap_or_default();
        let g = dir.join(s.trim().trim_start_matches("gitdir:").trim());
        match fs::read_to_string(g.join("commondir")) {
            Ok(c) => g.join(c.trim()),
            Err(_) => g,
        }
    } else {
        return "no git".into();
    };
    parse_origin(&fs::read_to_string(gitdir.join("config")).unwrap_or_default())
        .unwrap_or_else(|| "local only".into())
}

fn parse_origin(cfg: &str) -> Option<String> {
    let mut in_origin = false;
    for l in cfg.lines().map(str::trim) {
        if l.starts_with('[') {
            in_origin = l == "[remote \"origin\"]";
        } else if in_origin
            && let Some(("url", v)) = l.split_once('=').map(|(k, v)| (k.trim(), v.trim()))
        {
            return Some(strip_credentials(v));
        }
    }
    None
}

/// `https://user:token@host/x` -> `https://host/x`: tokens must not end up in the db or on screen.
fn strip_credentials(url: &str) -> String {
    if let Some((scheme, rest)) = url.split_once("://") {
        let host_end = rest.find('/').unwrap_or(rest.len());
        if let Some(at) = rest[..host_end].rfind('@') {
            return format!("{scheme}://{}", &rest[at + 1..]);
        }
    }
    url.to_string()
}

/// A console program run without its own window: once the GUI has no console, Windows would open one per call.
pub fn quiet_cmd(program: &str) -> std::process::Command {
    #[allow(unused_mut)]
    let mut cmd = std::process::Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

fn hostname() -> String {
    quiet_cmd("hostname")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".into())
}

#[derive(Clone, Serialize)]
pub struct Row {
    pub path: String,
    pub name: String,
    pub origin: String,
    pub description: String,
    pub missing: bool,
    pub tags: String,
    pub status: String,
    pub notes: String,
    pub tools: String,
    pub sessions: i64,
    pub first_seen: String,
    pub last_active: String,
    pub active_secs: f64,
    pub week_secs: f64,
    /// `status` if set by hand, else "active" (session in the last 14 days) or "idle".
    pub shown_status: String,
    /// Files with uncommitted changes; None when unknown (not a repo, git failed, folder gone).
    pub git_changed: Option<i64>,
    /// Commits on no remote; None when unknown.
    pub git_unpushed: Option<i64>,
    pub git_last_commit: Option<String>,
    pub stack: String,
    /// Other visible projects with the same origin repo.
    pub clones: usize,
    pub at_risk: bool,
}

const LIST_SQL: &str = "
WITH g AS (
  SELECT project, tool, session, ts,
    (julianday(ts) - julianday(LAG(ts) OVER (PARTITION BY tool, session ORDER BY ts))) * 86400 AS gap
  FROM events),
a AS (
  SELECT project, group_concat(DISTINCT tool) AS tools, count(DISTINCT tool || session) AS sessions,
    min(ts) AS first, max(ts) AS last,
    sum(CASE WHEN gap <= ?1 THEN gap ELSE 0 END) AS total,
    sum(CASE WHEN gap <= ?1 AND julianday(ts) >= julianday('now', '-7 days') THEN gap ELSE 0 END) AS week
  FROM g GROUP BY project)
SELECT p.path, p.name, p.origin, p.description, p.missing, p.tags, p.status, p.notes,
  coalesce(a.tools, ''), coalesce(a.sessions, 0), coalesce(a.first, p.first_seen), coalesce(a.last, ''),
  coalesce(a.total, 0), coalesce(a.week, 0),
  CASE WHEN p.status <> '' THEN p.status
       WHEN julianday(a.last) >= julianday('now', '-14 days') THEN 'active' ELSE 'idle' END,
  p.git_changed, p.git_unpushed, p.git_last_commit, p.stack
FROM projects p LEFT JOIN a ON a.project = p.path
ORDER BY coalesce(a.last, '') DESC, p.name";

/// All projects, newest activity first, excluded paths hidden (so editing excludes needs no re-import).
pub fn list(db: &Connection, excludes: &[String]) -> rusqlite::Result<Vec<Row>> {
    let mut st = db.prepare(LIST_SQL)?;
    let rows = st.query_map([IDLE_SECS], |r| {
        Ok(Row {
            path: r.get(0)?,
            name: r.get(1)?,
            origin: r.get(2)?,
            description: r.get(3)?,
            missing: r.get(4)?,
            tags: r.get(5)?,
            status: r.get(6)?,
            notes: r.get(7)?,
            tools: r.get(8)?,
            sessions: r.get(9)?,
            first_seen: r.get(10)?,
            last_active: r.get(11)?,
            active_secs: r.get(12)?,
            week_secs: r.get(13)?,
            shown_status: r.get(14)?,
            git_changed: r.get(15)?,
            git_unpushed: r.get(16)?,
            git_last_commit: r.get(17)?,
            stack: r.get(18)?,
            clones: 0,
            at_risk: false,
        })
    })?;
    let mut rows: Vec<Row> = rows
        .filter_map(Result::ok)
        .filter(|r| !excluded(&r.path, excludes))
        .collect();
    let mut per_repo: HashMap<String, usize> = HashMap::new();
    for k in rows.iter().filter_map(|r| origin_key(&r.origin)) {
        *per_repo.entry(k).or_default() += 1;
    }
    for r in &mut rows {
        r.clones = origin_key(&r.origin).map_or(0, |k| per_repo[&k] - 1);
        r.at_risk = at_risk(r);
    }
    Ok(rows)
}

/// Short git state for tables: "3 changed, 1 unpushed", "local only", "clean", "?" when unknown.
pub fn git_label(r: &Row) -> String {
    if r.missing {
        return String::new();
    }
    if r.origin == "no git" {
        return "no git".into();
    }
    let (Some(changed), Some(unpushed)) = (r.git_changed, r.git_unpushed) else {
        return "?".into();
    };
    let mut parts = vec![];
    if changed > 0 {
        parts.push(format!("{changed} changed"));
    }
    if unpushed > 0 && r.origin != "local only" {
        parts.push(format!("{unpushed} unpushed"));
    }
    if r.origin == "local only" {
        parts.push("local only".into());
    }
    if parts.is_empty() {
        "clean".into()
    } else {
        parts.join(", ")
    }
}

/// Work that exists in one place only: no git, no remote, uncommitted changes or unpushed commits.
pub fn at_risk(r: &Row) -> bool {
    !r.missing
        && (r.origin == "no git"
            || r.origin == "local only"
            || r.git_changed.is_some_and(|n| n > 0)
            || r.git_unpushed.is_some_and(|n| n > 0))
}

/// Same repo regardless of URL style: `git@h:a/b.git`, `https://h/a/b`, `ssh://u@h:22/a/b.git` -> `h/a/b`.
pub fn origin_key(origin: &str) -> Option<String> {
    let rest = if let Some((_, r)) = origin.split_once("://") {
        let (host, path) = r.split_once('/')?;
        let host = host.rsplit('@').next()?.split(':').next()?;
        format!("{host}/{path}")
    } else if let Some((user_host, path)) = origin.split_once(':') {
        // scp-style `git@host:a/b`; anything else without a scheme isn't a remote URL.
        if !user_host.contains('@') {
            return None;
        }
        format!("{}/{path}", user_host.rsplit('@').next()?)
    } else {
        return None;
    };
    let key = rest
        .trim_end_matches('/')
        .trim_end_matches(".git")
        .to_lowercase();
    key.contains('/').then_some(key)
}

/// (changed files, unpushed commits, last commit time) per repo, in parallel: git is slow to start on Windows.
fn git_health_all(repos: &[&String]) -> Vec<(Option<i64>, Option<i64>, Option<String>)> {
    let workers = std::thread::available_parallelism().map_or(4, |n| n.get().min(8));
    let chunk = repos.len().div_ceil(workers).max(1);
    std::thread::scope(|s| {
        let handles: Vec<_> = repos
            .chunks(chunk)
            .map(|c| {
                s.spawn(move || {
                    c.iter()
                        .map(|p| git_health(Path::new(p)))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().expect("git health worker panicked"))
            .collect()
    })
}

fn git_health(dir: &Path) -> (Option<i64>, Option<i64>, Option<String>) {
    // --no-optional-locks: never take index.lock in repos an agent may be working in.
    let git = |args: &[&str]| {
        let out = quiet_cmd("git")
            .arg("--no-optional-locks")
            .arg("-C")
            .arg(dir)
            .args(args)
            .output()
            .ok()?;
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
    };
    let changed = git(&["status", "--porcelain"]).map(|s| s.lines().count() as i64);
    let unpushed =
        git(&["rev-list", "--count", "HEAD", "--not", "--remotes"]).and_then(|s| s.parse().ok());
    let last = git(&["log", "-1", "--format=%cI"]).filter(|s| !s.is_empty());
    (changed, unpushed, last)
}

/// Languages and tools, from marker files in the project folder itself.
// shortcut: only the top folder is checked, so monorepos with code in subfolders show less; recurse if that matters.
pub fn stack(dir: &Path) -> String {
    const MARKERS: [(&str, &str); 12] = [
        ("Cargo.toml", "rust"),
        ("package.json", "node"),
        ("tsconfig.json", "typescript"),
        ("pyproject.toml", "python"),
        ("requirements.txt", "python"),
        ("go.mod", "go"),
        ("pom.xml", "java"),
        ("build.gradle", "java"),
        ("Gemfile", "ruby"),
        ("composer.json", "php"),
        ("Dockerfile", "docker"),
        ("pubspec.yaml", "dart"),
    ];
    let mut found: Vec<&str> = MARKERS
        .iter()
        .filter(|(f, _)| dir.join(f).is_file())
        .map(|(_, s)| *s)
        .collect();
    let exts: Vec<String> = fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .filter_map(|e| Some(e.path().extension()?.to_string_lossy().to_lowercase()))
                .collect()
        })
        .unwrap_or_default();
    for (ext, s) in [("tf", "terraform"), ("csproj", "dotnet"), ("sln", "dotnet")] {
        if exts.iter().any(|e| e == ext) {
            found.push(s);
        }
    }
    found.dedup();
    found.join(", ")
}

/// Merge a moved or renamed project into the one it became: history moves over, empty manual fields
/// are filled from the old project, and future log lines from the old path land on the new one.
pub fn merge(db: &mut Connection, from: &str, to: &str) -> rusqlite::Result<()> {
    if from == to {
        return Err(rusqlite::Error::InvalidParameterName(
            "can't merge a project into itself".into(),
        ));
    }
    let tx = db.transaction()?;
    let exists: bool =
        tx.query_row("SELECT count(*) FROM projects WHERE path = ?1", [to], |r| {
            r.get(0)
        })?;
    if !exists {
        return Err(rusqlite::Error::QueryReturnedNoRows);
    }
    tx.execute(
        "UPDATE events SET project = ?2 WHERE project = ?1",
        [from, to],
    )?;
    tx.execute(
        "UPDATE projects SET
           tags = CASE WHEN tags = '' THEN coalesce((SELECT tags FROM projects WHERE path = ?1), '') ELSE tags END,
           status = CASE WHEN status = '' THEN coalesce((SELECT status FROM projects WHERE path = ?1), '') ELSE status END,
           notes = CASE WHEN notes = '' THEN coalesce((SELECT notes FROM projects WHERE path = ?1), '') ELSE notes END,
           first_seen = min(first_seen, coalesce((SELECT first_seen FROM projects WHERE path = ?1), first_seen))
         WHERE path = ?2",
        [from, to],
    )?;
    tx.execute("DELETE FROM projects WHERE path = ?1", [from])?;
    tx.execute(
        "UPDATE moves SET to_path = ?2 WHERE to_path = ?1",
        [from, to],
    )?;
    tx.execute(
        "INSERT OR REPLACE INTO moves(from_path, to_path) VALUES (?1, ?2)",
        [from, to],
    )?;
    tx.commit()
}

/// RFC 4180 CSV of the rows, for spreadsheets and other tools.
pub fn csv(rows: &[Row]) -> String {
    let q = |s: &str| {
        if s.contains([',', '"', '\n', '\r']) {
            format!("\"{}\"", s.replace('"', "\"\""))
        } else {
            s.to_string()
        }
    };
    let opt = |n: Option<i64>| n.map_or(String::new(), |n| n.to_string());
    let mut out = String::from(
        "path,name,status,tags,notes,origin,clones,stack,git_changed,git_unpushed,git_last_commit,at_risk,missing,tools,sessions,active_hours,week_hours,first_seen,last_active,description\r\n",
    );
    for r in rows {
        let fields = [
            q(&r.path),
            q(&r.name),
            q(&r.shown_status),
            q(&r.tags),
            q(&r.notes),
            q(&r.origin),
            r.clones.to_string(),
            q(&r.stack),
            opt(r.git_changed),
            opt(r.git_unpushed),
            q(r.git_last_commit.as_deref().unwrap_or("")),
            r.at_risk.to_string(),
            r.missing.to_string(),
            q(&r.tools),
            r.sessions.to_string(),
            format!("{:.2}", r.active_secs / 3600.0),
            format!("{:.2}", r.week_secs / 3600.0),
            q(&r.first_seen),
            q(&r.last_active),
            q(&r.description),
        ];
        out.push_str(&fields.join(","));
        out.push_str("\r\n");
    }
    out
}

pub fn save_manual(db: &Connection, r: &Row) -> rusqlite::Result<()> {
    db.execute(
        "UPDATE projects SET tags = ?2, status = ?3, notes = ?4 WHERE path = ?1",
        params![r.path, r.tags, r.status, r.notes],
    )?;
    Ok(())
}

/// Drop one project and its history from the database (the folder and logs are untouched).
pub fn forget(db: &Connection, path: &str) -> rusqlite::Result<()> {
    db.execute("DELETE FROM events WHERE project = ?1", [path])?;
    db.execute("DELETE FROM projects WHERE path = ?1", [path])?;
    Ok(())
}

/// Forget every project whose folder is gone. Returns how many.
pub fn forget_missing(db: &mut Connection) -> rusqlite::Result<usize> {
    let tx = db.transaction()?;
    tx.execute(
        "DELETE FROM events WHERE project IN (SELECT path FROM projects WHERE missing = 1)",
        [],
    )?;
    let n = tx.execute("DELETE FROM projects WHERE missing = 1", [])?;
    tx.commit()?;
    Ok(n)
}

/// Session logs not modified in `days` days, with their total size in bytes.
pub fn old_logs(home: &Path, days: u64) -> (Vec<PathBuf>, u64) {
    let cutoff = std::time::SystemTime::now() - std::time::Duration::from_secs(days * 86400);
    let mut size = 0;
    let files = log_files(home)
        .into_iter()
        .filter(|(tool, _)| *tool != "hermes") // a live SQLite store, not a log file
        .map(|(_, f)| f)
        .filter(|f| {
            let Ok(m) = fs::metadata(f) else { return false };
            let old = m.modified().is_ok_and(|t| t < cutoff);
            if old {
                size += m.len();
            }
            old
        })
        .collect();
    (files, size)
}

/// Move logs to the Recycle Bin / Trash, plus Claude's `<session>/` folder (subagents, tool results) next to each.
/// Hangar keeps the imported history, so the projects' time stays in the list.
pub fn trash_logs(files: &[PathBuf]) -> Result<usize, trash::Error> {
    let mut all = files.to_vec();
    all.extend(
        files
            .iter()
            .map(|f| f.with_extension(""))
            .filter(|d| d.is_dir()),
    );
    trash::delete_all(&all)?;
    Ok(files.len())
}

/// Why a project folder must not be trashed, if it mustn't: home, its parents, a scan root, or the AI tools' own data.
pub fn delete_guard(path: &str, home: &Path, roots: &[String]) -> Option<&'static str> {
    let p = Path::new(path);
    if !p.is_dir() {
        Some("folder doesn't exist")
    } else if home.starts_with(p) || p.parent().is_none() {
        Some("this is your home folder or one of its parents")
    } else if roots.iter().any(|r| Path::new(r).starts_with(p)) {
        Some("this is (a parent of) a scan root")
    } else if p.starts_with(home.join(".claude")) || p.starts_with(home.join(".codex")) {
        Some("this is AI tool data")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delete_guard_protects_home_and_roots() {
        let home = std::env::temp_dir();
        let root = home.join("hangar-guard-root");
        let proj = root.join("proj");
        fs::create_dir_all(&proj).unwrap();
        let roots = vec![root.to_string_lossy().to_string()];
        let s = |p: &Path| p.to_string_lossy().to_string();
        assert!(delete_guard(&s(&home), &home, &roots).is_some());
        assert!(delete_guard(&s(home.parent().unwrap()), &home, &roots).is_some());
        assert!(delete_guard(&s(&root), &home, &roots).is_some());
        assert!(delete_guard(&s(&home.join(".claude").join("x")), &home, &roots).is_some());
        assert_eq!(delete_guard(&s(&proj), &home, &roots), None);
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn status_migrates_and_derives() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(SCHEMA).unwrap();
        for (p, status) in [("/old", "active"), ("/paused", "paused")] {
            db.execute(
                "INSERT INTO projects(path, name, origin, description, missing, host, first_seen, status) VALUES (?1,'n','','',0,'h','',?2)",
                [p, status],
            )
            .unwrap();
        }
        db.execute(
            "INSERT INTO events VALUES ('f','/recent','claude','s',strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
            [],
        )
        .unwrap();
        db.execute(
            "INSERT INTO projects(path, name, origin, description, missing, host, first_seen) VALUES ('/recent','n','','',0,'h','')",
            [],
        )
        .unwrap();
        migrate(&db).unwrap();
        let shown: HashMap<String, String> = list(&db, &[])
            .unwrap()
            .into_iter()
            .map(|r| (r.path, r.shown_status))
            .collect();
        assert_eq!(shown["/old"], "idle");
        assert_eq!(shown["/paused"], "paused");
        assert_eq!(shown["/recent"], "active");
    }

    fn test_db() -> Connection {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(SCHEMA).unwrap();
        migrate(&db).unwrap();
        db
    }

    fn add_project(db: &Connection, path: &str, origin: &str) {
        db.execute(
            "INSERT INTO projects(path, name, origin, description, missing, host, first_seen) VALUES (?1,'n',?2,'',0,'h','2026-01-01')",
            [path, origin],
        )
        .unwrap();
    }

    #[test]
    fn migration_from_v1_adds_columns() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(SCHEMA).unwrap();
        db.execute_batch("PRAGMA user_version = 1;").unwrap();
        add_project(&db, "/p", "no git");
        migrate(&db).unwrap();
        migrate(&db).unwrap(); // idempotent: a second launch must not fail on duplicate columns
        let v: i64 = db
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, 2);
        assert_eq!(list(&db, &[]).unwrap()[0].git_changed, None);
    }

    #[test]
    fn origin_keys_match_across_url_styles() {
        let key = Some("github.com/acme/app".to_string());
        for url in [
            "git@github.com:acme/app.git",
            "https://github.com/acme/app",
            "https://github.com/Acme/App.git/",
            "ssh://git@github.com:22/acme/app.git",
            "https://user:tok@github.com/acme/app.git",
        ] {
            assert_eq!(origin_key(url), key, "{url}");
        }
        assert_eq!(origin_key("local only"), None);
        assert_eq!(origin_key("no git"), None);
        assert_eq!(origin_key(r"C:\repos\app.git"), None);
    }

    #[test]
    fn clones_and_risk_are_computed() {
        let db = test_db();
        add_project(&db, "/a", "git@github.com:acme/app.git");
        add_project(&db, "/b", "https://github.com/acme/app");
        add_project(&db, "/c", "local only");
        db.execute(
            "UPDATE projects SET git_changed = 0, git_unpushed = 2 WHERE path = '/a'",
            [],
        )
        .unwrap();
        db.execute(
            "UPDATE projects SET git_changed = 0, git_unpushed = 0 WHERE path = '/b'",
            [],
        )
        .unwrap();
        let rows: HashMap<String, Row> = list(&db, &[])
            .unwrap()
            .into_iter()
            .map(|r| (r.path.clone(), r))
            .collect();
        assert_eq!(
            (rows["/a"].clones, rows["/b"].clones, rows["/c"].clones),
            (1, 1, 0)
        );
        assert!(rows["/a"].at_risk && !rows["/b"].at_risk && rows["/c"].at_risk);
        assert_eq!(git_label(&rows["/a"]), "2 unpushed");
        assert_eq!(git_label(&rows["/b"]), "clean");
        assert_eq!(git_label(&rows["/c"]), "?");
    }

    #[test]
    fn merge_moves_history_and_aliases_old_paths() {
        let mut db = test_db();
        add_project(&db, "/old/app", "https://h/a/app");
        add_project(&db, "/new/app", "https://h/a/app");
        db.execute(
            "UPDATE projects SET tags = 'keep', first_seen = '2025-01-01' WHERE path = '/old/app'",
            [],
        )
        .unwrap();
        db.execute(
            "INSERT INTO events VALUES ('f','/old/app','claude','s','2026-01-01T00:00:00.000Z')",
            [],
        )
        .unwrap();
        merge(&mut db, "/old/app", "/new/app").unwrap();
        let rows = list(&db, &[]).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(
            (
                rows[0].path.as_str(),
                rows[0].tags.as_str(),
                rows[0].sessions
            ),
            ("/new/app", "keep", 1)
        );
        assert_eq!(rows[0].first_seen, "2026-01-01T00:00:00.000Z"); // from events, which moved over
        let moves = vec![("/old/app".to_string(), "/new/app".to_string())];
        assert_eq!(
            Path::new(&apply_moves("/old/app/src", &moves)),
            Path::new("/new/app/src")
        );
        assert_eq!(apply_moves("/old/app", &moves), "/new/app");
        assert_eq!(apply_moves("/old/apple", &moves), "/old/apple");
        assert!(merge(&mut db, "/new/app", "/new/app").is_err());
        // A later merge of the target keeps the chain pointing at the final project.
        add_project(&db, "/final/app", "https://h/a/app");
        merge(&mut db, "/new/app", "/final/app").unwrap();
        let to: String = db
            .query_row(
                "SELECT to_path FROM moves WHERE from_path = '/old/app'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(to, "/final/app");
    }

    #[test]
    fn csv_quotes_awkward_fields() {
        let db = test_db();
        add_project(&db, "/p", "no git");
        db.execute(
            "UPDATE projects SET notes = 'a, \"b\"' || char(10) || 'c' WHERE path = '/p'",
            [],
        )
        .unwrap();
        let out = csv(&list(&db, &[]).unwrap());
        assert!(out.contains("\"a, \"\"b\"\"\nc\""), "{out}");
        assert_eq!(out.matches("\r\n").count(), 2);
    }

    #[test]
    fn missing_folder_keeps_last_known_facts() {
        let home = std::env::temp_dir().join("hangar-missing-test");
        let gone = home.join("gone-project");
        let _ = fs::remove_dir_all(&home);
        fs::create_dir_all(&home).unwrap();
        let mut db = test_db();
        add_project(&db, &gone.to_string_lossy(), "https://h/a/gone");
        db.execute(
            "UPDATE projects SET description = 'kept' WHERE missing = 0",
            [],
        )
        .unwrap();
        db.execute(
            "INSERT INTO events VALUES ('f', ?1, 'claude', 's', '2026-01-01T00:00:00.000Z')",
            [gone.to_string_lossy()],
        )
        .unwrap();
        run(&mut db, &home, &[], &[]).unwrap();
        let r = &list(&db, &[]).unwrap()[0];
        assert!(r.missing);
        assert_eq!(
            (r.origin.as_str(), r.description.as_str()),
            ("https://h/a/gone", "kept")
        );
        fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn git_health_reads_a_real_repo() {
        let dir = std::env::temp_dir().join("hangar-git-health-test");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let git = |args: &[&str]| {
            std::process::Command::new("git")
                .args([
                    "-c",
                    "user.name=t",
                    "-c",
                    "user.email=t@t",
                    "-c",
                    "commit.gpgsign=false",
                    "-C",
                ])
                .arg(&dir)
                .args(args)
                .output()
                .unwrap()
        };
        git(&["init", "-q"]);
        fs::write(dir.join("a.txt"), "a").unwrap();
        git(&["add", "a.txt"]);
        git(&["commit", "-q", "-m", "one"]);
        fs::write(dir.join("b.txt"), "b").unwrap();
        let (changed, unpushed, last) = git_health(&dir);
        assert_eq!((changed, unpushed), (Some(1), Some(1)));
        assert!(last.is_some());
        assert_eq!(git_health(&dir.join("nope")), (None, None, None));
        fs::write(dir.join("Cargo.toml"), "").unwrap();
        fs::write(dir.join("main.tf"), "").unwrap();
        assert_eq!(stack(&dir), "rust, terraform");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn gemini_and_hermes_are_imported() {
        let home = std::env::temp_dir().join("hangar-tools-test");
        let _ = fs::remove_dir_all(&home);
        let proj = home.join("proj");
        fs::create_dir_all(&proj).unwrap();
        let p = proj.to_string_lossy().to_string();
        // Gemini: key dir found through projects.json, one subagent session that must be ignored.
        let chats = home.join(".gemini").join("tmp").join("proj").join("chats");
        fs::create_dir_all(&chats).unwrap();
        fs::write(
            home.join(".gemini").join("projects.json"),
            serde_json::json!({"projects": {p.clone(): "proj"}}).to_string(),
        )
        .unwrap();
        fs::write(
            chats.join("session-2026-01-01T10-00-aaaa.jsonl"),
            [
                r#"{"sessionId":"g1","projectHash":"x","startTime":"2026-01-01T10:00:00.000Z","lastUpdated":"2026-01-01T10:05:00.000Z","kind":"main"}"#,
                r#"{"id":"1","timestamp":"2026-01-01T10:01:00.000Z","type":"user","content":"hi"}"#,
                r#"{"$set":{"summary":"x"}}"#,
                r#"{"id":"2","timestamp":"2026-01-01T10:05:00.000Z","type":"gemini","content":"yo"}"#,
            ]
            .join("\n"),
        )
        .unwrap();
        fs::write(
            chats.join("session-2026-01-01T11-00-bbbb.jsonl"),
            r#"{"sessionId":"g2","startTime":"2026-01-01T11:00:00.000Z","kind":"subagent"}"#
                .to_string()
                + "\n"
                + r#"{"id":"1","timestamp":"2026-01-01T11:01:00.000Z","type":"user"}"#,
        )
        .unwrap();
        // Hermes: documented sessions/messages tables, epoch seconds.
        let hermes = home.join("hermes");
        fs::create_dir_all(&hermes).unwrap();
        let h = Connection::open(hermes.join("state.db")).unwrap();
        h.execute_batch(
            "CREATE TABLE sessions(id TEXT PRIMARY KEY, source TEXT, started_at REAL, cwd TEXT);
             CREATE TABLE messages(id INTEGER PRIMARY KEY, session_id TEXT, role TEXT, timestamp REAL);
             INSERT INTO messages(session_id, role, timestamp) VALUES ('h1','user',1767261600.0),('h1','assistant',1767261660.0),('h2','user',1767261600.0);",
        )
        .unwrap();
        h.execute("INSERT INTO sessions VALUES ('h1','cli',1767261600.0,?1),('h2','gateway',1767261600.0,NULL)", [&p]).unwrap();
        drop(h);
        // SAFETY: test-only; HERMES_HOME is read by log_files and no other test sets it.
        unsafe { std::env::set_var("HERMES_HOME", &hermes) };
        let mut db = test_db();
        run(&mut db, &home, &[], &[]).unwrap();
        unsafe { std::env::remove_var("HERMES_HOME") };
        let r = &list(&db, &[]).unwrap()[0];
        assert_eq!(r.path, normalize(&p));
        assert_eq!(
            (r.tools.as_str(), r.sessions),
            ("gemini,hermes", 2),
            "{}",
            r.tools
        );
        assert!((r.active_secs - 360.0).abs() < 0.01, "{}", r.active_secs); // 5 min Gemini + 1 min Hermes
        // The log cleaner leaves the Hermes database alone.
        let (old, _) = old_logs(&home, 0);
        assert!(
            old.iter()
                .all(|f| f.extension().is_some_and(|e| e == "jsonl"))
        );
        fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn excludes_match_whole_segments() {
        let ex = vec!["Temp".to_string(), "plugins/cache".to_string()];
        assert!(excluded(r"C:\Users\x\AppData\Local\Temp\claude", &ex));
        assert!(!excluded(r"C:\src\templates", &ex));
        assert!(excluded("/Users/x/.claude/plugins/cache/foo", &ex));
        assert!(!excluded("/Users/x/plugins/other/cache", &ex));
    }

    #[test]
    fn normalize_unifies_spelling() {
        let dir = std::env::temp_dir();
        let a = normalize(&dir.to_string_lossy());
        let b = normalize(&format!("{}/", dir.to_string_lossy()));
        assert_eq!(a, b);
        if cfg!(windows) {
            assert_eq!(normalize(&dir.to_string_lossy().to_lowercase()), a);
            assert_eq!(normalize(r"c:\does\not\exist\"), r"C:\does\not\exist");
        }
    }

    #[test]
    fn paragraph_skips_noise() {
        let md = "---\nname: x\n---\n# Title\n![badge](x)\n\nThe real\ndescription.\n\nMore.";
        assert_eq!(first_paragraph(md), "The real description.");
        assert_eq!(first_paragraph("```\ncode\n```\nText"), "Text");
        assert_eq!(
            first_paragraph(
                "**Fast.** A [CLI](https://example.com) for [Foo](https://foo.example) `v2` [sic]."
            ),
            "Fast. A CLI for Foo v2 [sic]."
        );
    }

    #[test]
    fn origin_parsing() {
        let cfg = "[core]\n\turl = nope\n[remote \"origin\"]\n\turl = https://me:tok@github.com/a/b.git\n";
        assert_eq!(
            parse_origin(cfg).as_deref(),
            Some("https://github.com/a/b.git")
        );
        assert_eq!(parse_origin("[remote \"upstream\"]\nurl = x"), None);
        assert_eq!(
            strip_credentials("git@github.com:a/b.git"),
            "git@github.com:a/b.git"
        );
    }

    #[test]
    fn codex_tracks_session_and_cwd() {
        let mut c = Codex::default();
        let lines = [
            r#"{"timestamp":"2026-08-06T18:46:19.577Z","type":"session_meta","payload":{"id":"s1","cwd":"/a"}}"#,
            r#"{"timestamp":"2026-08-06T18:47:00.000Z","type":"turn_context","payload":{"cwd":"/b"}}"#,
            r#"{"timestamp":"2026-08-06T18:48:00.000Z","type":"session_meta","payload":{"id":"s2","cwd":"/c"}}"#,
        ];
        let got: Vec<_> = lines
            .iter()
            .filter_map(|l| c.feed(&serde_json::from_str(l).unwrap()))
            .collect();
        assert_eq!(got[1].0, "/b");
        assert_eq!((got[2].0.as_str(), got[2].1.as_str()), ("/c", "s2"));
    }

    #[test]
    fn active_time_drops_idle_gaps() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(SCHEMA).unwrap();
        migrate(&db).unwrap();
        db.execute(
            "INSERT INTO projects(path, name, origin, description, missing, host, first_seen) VALUES ('/p','p','','',0,'h','')",
            [],
        )
        .unwrap();
        // Real log formats: 10 min of work, a 2 h idle gap, 5 more min.
        for ts in [
            "2026-08-06T10:00:00.000Z",
            "2026-08-06T10:10:00.000Z",
            "2026-08-06T12:10:00.000Z",
            "2026-08-06T12:15:00.589Z",
        ] {
            db.execute("INSERT INTO events VALUES ('f','/p','claude','s',?1)", [ts])
                .unwrap();
        }
        let r = &list(&db, &[]).unwrap()[0];
        assert!((r.active_secs - 900.589).abs() < 0.01, "{}", r.active_secs);
        assert_eq!(r.sessions, 1);
        assert_eq!(r.last_active, "2026-08-06T12:15:00.589Z");
    }
}
