use std::path::{Path, PathBuf};

use super::history::{claude_folders, codex_files};
use super::*;
use crate::activity::{claude_project_key, claude_projects_dir};

fn transcript(dir: &Path, cwd: &Path) {
    std::fs::create_dir_all(dir).unwrap();
    let line = serde_json::json!({ "type": "user", "cwd": cwd.display().to_string() });
    std::fs::write(dir.join("s.jsonl"), format!("{line}\n")).unwrap();
}

fn names(found: &[PathBuf]) -> Vec<String> {
    found
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect()
}

#[test]
fn a_bots_history_is_its_exact_key_and_sessions_started_inside_it() {
    let home = tempfile::tempdir().unwrap();
    let ws = home
        .path()
        .join(".flotilla/projects/acme/bots/dev/workspace");
    let other = home
        .path()
        .join(".flotilla/projects/acme/bots/dev/workspace/x/workspace");
    let claude = claude_projects_dir(home.path());
    let key = claude_project_key(&ws);
    std::fs::create_dir_all(claude.join(&key)).unwrap();
    // Started in a folder of the workspace: the bot's.
    transcript(&claude.join(format!("{key}-repo")), &ws.join("repo"));
    // Same prefix, started elsewhere (a lossy key): not the bot's.
    transcript(
        &claude.join(format!("{key}-old")),
        Path::new("/elsewhere/old"),
    );
    // Same prefix, nothing says where: not the bot's.
    std::fs::create_dir_all(claude.join(format!("{key}-unknown"))).unwrap();
    // Another bot's workspace whose key starts with this one's.
    let theirs = claude_project_key(&other);
    transcript(&claude.join(&theirs), &other);
    // A sibling whose key only shares the text, not a `-` boundary.
    std::fs::create_dir_all(claude.join(format!("{key}2"))).unwrap();
    let found = claude_folders(home.path(), &ws, &[ws.clone(), other.clone()]);
    assert_eq!(names(&found), vec![key.clone(), format!("{key}-repo")]);
}

#[test]
fn a_codex_bots_history_is_its_threads_session_file() {
    let home = tempfile::tempdir().unwrap();
    let bot_dir = home.path().join("bot");
    std::fs::create_dir_all(&bot_dir).unwrap();
    std::fs::write(bot_dir.join("codex-thread-id"), "abc-123\n").unwrap();
    let day = home.path().join(".codex/sessions/2026/10/16");
    std::fs::create_dir_all(&day).unwrap();
    std::fs::write(day.join("rollout-2026-10-16T08-00-00-abc-123.jsonl"), "{}").unwrap();
    std::fs::write(day.join("rollout-2026-10-16T09-00-00-zzz-999.jsonl"), "{}").unwrap();
    let found = codex_files(home.path(), &bot_dir);
    assert_eq!(
        names(&found),
        vec!["rollout-2026-10-16T08-00-00-abc-123.jsonl"]
    );

    std::fs::write(bot_dir.join("codex-thread-id"), "../../etc").unwrap();
    assert!(
        codex_files(home.path(), &bot_dir).is_empty(),
        "not a thread id"
    );
}

#[test]
fn sizes_read_as_the_owner_reads_them() {
    assert_eq!(size_text(1_600_000_000), "1.6 GB");
    assert_eq!(size_text(380_000_000), "380 MB");
    assert_eq!(size_text(4_000), "4 KB");
    assert_eq!(
        groups_text(&[
            (Group::ProjectFiles, 380_000_000),
            (Group::GitCache, 3_000_000)
        ]),
        "project files 380 MB, git cache 3 MB"
    );
}
