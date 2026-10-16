//! A project with two bots, and what its delete leaves to look at
//! (0.19.0): Claude Code history folders, a git cache, the trash folder.

use std::path::{Path, PathBuf};

use flotillad::trash::{bucket_dir, Bucket};
use serde_json::{json, Value};

use super::{create_bot, spawn_daemon_with, TestDaemon, WsClient};

/// Claude Code's folder name for sessions started in `path`.
pub fn key(path: &Path) -> String {
    path.to_string_lossy()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

pub fn claude(d: &TestDaemon) -> PathBuf {
    d.app.cfg.user_home.join(".claude/projects")
}

/// A history folder for `cwd` holding `bytes` of transcript.
pub fn history(d: &TestDaemon, cwd: &Path, bytes: usize) -> PathBuf {
    let dir = claude(d).join(key(cwd));
    std::fs::create_dir_all(&dir).unwrap();
    let line = json!({ "type": "user", "cwd": cwd.display().to_string() }).to_string();
    let mut text = format!("{line}\n");
    text.push_str(&"x".repeat(bytes.saturating_sub(text.len())));
    std::fs::write(dir.join("session.jsonl"), text).unwrap();
    dir
}

pub fn trashed(d: &TestDaemon, project_dir: &str, group: &str) -> PathBuf {
    bucket_dir(
        &d.app.cfg,
        Bucket {
            label: project_dir,
            group,
        },
    )
}

pub struct Acme {
    pub d: TestDaemon,
    pub c: WsClient,
    pub id: String,
    pub dir: String,
    pub workspaces: Vec<PathBuf>,
}

pub async fn acme() -> Acme {
    acme_with(|_| {}).await
}

pub async fn acme_with(tweak: impl FnOnce(&mut flotillad::config::Config)) -> Acme {
    let d = spawn_daemon_with(tweak).await;
    let mut c = WsClient::connect(&d).await;
    let created = c
        .request(json!({ "type": "create_project", "name": "Acme" }))
        .await;
    let id = created["project"]["id"].as_str().unwrap().to_string();
    let dir = created["project"]["dir_name"].as_str().unwrap().to_string();
    let mut workspaces = Vec::new();
    for name in ["dev", "ops"] {
        let bot = create_bot(&mut c, &id, name).await;
        workspaces.push(PathBuf::from(bot["workspace_path"].as_str().unwrap()));
    }
    Acme {
        d,
        c,
        id,
        dir,
        workspaces,
    }
}

pub async fn delete(c: &mut WsClient, id: &str) {
    let reply = c
        .request(json!({ "type": "delete_project", "project_id": id }))
        .await;
    assert_eq!(reply["type"], "ok", "{reply}");
}

pub async fn preview(c: &mut WsClient, id: &str) -> Value {
    let reply = c
        .request(json!({ "type": "project_delete_preview", "project_id": id }))
        .await;
    assert_eq!(reply["type"], "project_delete_preview", "{reply}");
    reply["preview"].clone()
}

pub fn group<'a>(preview: &'a Value, name: &str) -> &'a Value {
    preview["groups"]
        .as_array()
        .unwrap()
        .iter()
        .find(|g| g["group"] == name)
        .unwrap_or_else(|| panic!("no {name} in {preview}"))
}

pub fn git_cache(a: &Acme) -> PathBuf {
    let cache = a.d.app.cfg.home.join(format!("cache/repos/{}.git", a.id));
    std::fs::create_dir_all(&cache).unwrap();
    std::fs::write(cache.join("HEAD"), vec![b'r'; 3_000]).unwrap();
    cache
}

/// Points a bot's stored workspace somewhere else, as a copied or tampered
/// database would.
pub fn craft_workspace(d: &TestDaemon, bot_id: &str, path: &Path) {
    let db = rusqlite::Connection::open(d.app.cfg.db_path()).unwrap();
    db.execute(
        "UPDATE bot SET workspace_path = ?1 WHERE id = ?2",
        rusqlite::params![path.display().to_string(), bot_id],
    )
    .unwrap();
}
