//! A bot's session history outside its folder.
//!
//! Claude Code files a session started in `<dir>` under
//! `~/.claude/projects/<key>`, the key being the path with every character
//! that isn't an ASCII letter or digit replaced by `-`
//! ([`crate::activity::claude_project_key`]). The daemon starts each bot's
//! sessions in its workspace, so that key is the bot's. A session the bot
//! itself started in a folder of its workspace has `<key>-<rest>`.
//!
//! The key is lossy (`a/b-c` and `a/b/c` share one), so a longer key is
//! taken only when its transcripts say they were started inside the
//! workspace, and never when it is the exact key of another bot's
//! workspace or begins with one.
//!
//! Codex keeps its sessions in `~/.codex/sessions/<y>/<m>/<d>/
//! rollout-<time>-<thread>.jsonl`; the bot folder names the thread it is on
//! (`codex-thread-id`), and that thread's file is the bot's.

use std::io::BufRead;
use std::path::{Path, PathBuf};

use crate::activity::{claude_project_key, claude_projects_dir};

/// How many lines of a transcript are read for the folder it started in.
const CWD_LINES: usize = 40;
/// How many transcripts of a candidate are looked at.
const CWD_FILES: usize = 5;

/// The history folders of the bot whose workspace is `workspace`, in
/// `user_home`. `others` are the workspaces of every other bot this
/// computer knows, archived ones included.
pub fn claude_folders(user_home: &Path, workspace: &Path, others: &[PathBuf]) -> Vec<PathBuf> {
    let key = claude_project_key(workspace);
    let other_keys: Vec<String> = others
        .iter()
        .filter(|w| w.as_path() != workspace)
        .map(|w| claude_project_key(w))
        .filter(|k| *k != key)
        .collect();
    let root = claude_projects_dir(user_home);
    let Ok(entries) = std::fs::read_dir(&root) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name == key {
            found.push(entry.path());
            continue;
        }
        let Some(rest) = name.strip_prefix(&key) else {
            continue;
        };
        if !rest.starts_with('-') {
            continue;
        }
        let another = other_keys.iter().any(|o| {
            name == *o || (name.starts_with(o.as_str()) && name[o.len()..].starts_with('-'))
        });
        if !another && started_inside(&entry.path(), workspace) {
            found.push(entry.path());
        }
    }
    found.sort();
    found
}

/// Whether the transcripts in `dir` were started inside `workspace`: at
/// least one says so, and none says it was elsewhere.
fn started_inside(dir: &Path, workspace: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    let mut inside = false;
    for path in entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "jsonl"))
        .take(CWD_FILES)
    {
        match first_cwd(&path) {
            Some(cwd) if Path::new(&cwd).starts_with(workspace) => inside = true,
            Some(_) => return false,
            None => {}
        }
    }
    inside
}

/// The `cwd` the transcript records first.
fn first_cwd(path: &Path) -> Option<String> {
    let file = std::fs::File::open(path).ok()?;
    std::io::BufReader::new(file)
        .lines()
        .take(CWD_LINES)
        .map_while(Result::ok)
        .find_map(|line| {
            serde_json::from_str::<serde_json::Value>(&line)
                .ok()?
                .get("cwd")?
                .as_str()
                .map(str::to_string)
        })
}

/// Where Codex keeps its sessions for `user_home`.
pub fn codex_sessions_dir(user_home: &Path) -> PathBuf {
    user_home.join(".codex").join("sessions")
}

/// The Codex session files of the thread named in `bot_dir`.
pub fn codex_files(user_home: &Path, bot_dir: &Path) -> Vec<PathBuf> {
    let Some(raw) =
        crate::paths::no_follow::read(bot_dir, Path::new(crate::runtime::codex::THREAD_FILE))
    else {
        return Vec::new();
    };
    let thread = raw.trim().to_string();
    let plain = !thread.is_empty()
        && thread
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-');
    if !plain {
        return Vec::new();
    }
    let suffix = format!("-{thread}.jsonl");
    let mut found = Vec::new();
    // `sessions/<y>/<m>/<d>/<file>`: four levels, never followed through a
    // link.
    let mut stack = vec![(codex_sessions_dir(user_home), 0usize)];
    while let Some((dir, depth)) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(meta) = std::fs::symlink_metadata(entry.path()) else {
                continue;
            };
            if crate::cleanup::scope::is_link(&meta) {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            if meta.is_dir() && depth < 3 {
                stack.push((entry.path(), depth + 1));
            } else if meta.is_file() && name.starts_with("rollout-") && name.ends_with(&suffix) {
                found.push(entry.path());
            }
        }
    }
    found.sort();
    found
}
