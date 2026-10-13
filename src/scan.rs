use rusqlite::{Connection, params};
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
  tags TEXT NOT NULL DEFAULT '', status TEXT NOT NULL DEFAULT 'active', notes TEXT NOT NULL DEFAULT '');
CREATE TABLE IF NOT EXISTS events(file TEXT NOT NULL, project TEXT NOT NULL, tool TEXT NOT NULL,
  session TEXT NOT NULL, ts TEXT NOT NULL);
CREATE INDEX IF NOT EXISTS events_file ON events(file);
CREATE INDEX IF NOT EXISTS events_session ON events(tool, session, ts);
CREATE TABLE IF NOT EXISTS files(path TEXT PRIMARY KEY, mtime INTEGER NOT NULL, size INTEGER NOT NULL);
";

pub fn open(path: &Path) -> rusqlite::Result<Connection> {
    let db = Connection::open(path)?;
    db.execute_batch(SCHEMA)?;
    Ok(db)
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
    let mut cache = HashMap::new();
    let mut files = 0;
    let mut logs = Vec::new();
    for dir in read_dirs(&home.join(".claude").join("projects")) {
        logs.extend(jsonl_in(&dir).into_iter().map(|f| ("claude", f)));
    }
    logs.extend(
        jsonl_tree(&home.join(".codex").join("sessions"))
            .into_iter()
            .map(|f| ("codex", f)),
    );
    for (tool, file) in logs {
        let Ok(meta) = fs::metadata(&file) else {
            continue;
        };
        let mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_secs() as i64);
        let size = meta.len() as i64;
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
        import_file(&tx, &file, &key, tool, &mut cache)?;
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
        let summary = description_file(dir)
            .map(|f| first_paragraph(&fs::read_to_string(f).unwrap_or_default()));
        // Upsert touches automatic columns only: tags/status/notes are the user's.
        tx.execute(
            "INSERT INTO projects(path, name, origin, description, missing, host, first_seen)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
             ON CONFLICT(path) DO UPDATE SET name = ?2, origin = ?3, description = ?4, missing = ?5, host = ?6",
            params![p, name, origin(dir), summary.unwrap_or_default(), !dir.exists(), host],
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
    cache: &mut HashMap<String, String>,
) -> rusqlite::Result<()> {
    let Ok(f) = fs::File::open(file) else {
        return Ok(());
    };
    let mut codex = Codex::default();
    let mut ins = tx.prepare_cached(
        "INSERT INTO events(file, project, tool, session, ts) VALUES (?1, ?2, ?3, ?4, ?5)",
    )?;
    for line in BufReader::new(f).lines() {
        // Unparseable lines (partial writes, unknown shapes) are skipped.
        let Ok(line) = line else { continue };
        let Ok(v) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let ev = if tool == "claude" {
            claude_event(&v)
        } else {
            codex.feed(&v)
        };
        let Some((cwd, session, ts)) = ev else {
            continue;
        };
        let project = cache
            .entry(cwd.clone())
            .or_insert_with(|| project_root(&cwd));
        ins.execute(params![key, project.as_str(), tool, session, ts])?;
    }
    Ok(())
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

fn hostname() -> String {
    std::process::Command::new("hostname")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".into())
}

#[derive(Clone)]
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
  coalesce(a.total, 0), coalesce(a.week, 0)
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
        })
    })?;
    Ok(rows
        .filter_map(Result::ok)
        .filter(|r| !excluded(&r.path, excludes))
        .collect())
}

pub fn save_manual(db: &Connection, r: &Row) -> rusqlite::Result<()> {
    db.execute(
        "UPDATE projects SET tags = ?2, status = ?3, notes = ?4 WHERE path = ?1",
        params![r.path, r.tags, r.status, r.notes],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
