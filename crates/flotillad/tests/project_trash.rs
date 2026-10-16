//! Deleting a project moves what it left on disk to the trash (0.19.0):
//! its folder, its bots' Claude Code history, its git cache, and never what
//! a running process still holds. Test daemons always use the trash folder
//! in their own home, so that is where these look.

use std::path::{Path, PathBuf};

use crate::common;

use common::peers::wait_until;
use common::trash::{acme, delete, git_cache, group, history, preview, trashed};
use common::WsClient;
use serde_json::json;

#[tokio::test]
async fn deleting_a_project_moves_its_folder_history_and_git_cache_to_the_trash() {
    let mut a = acme().await;
    let project_dir = a.d.app.cfg.projects_dir().join(&a.dir);
    let histories: Vec<PathBuf> = a
        .workspaces
        .iter()
        .map(|w| history(&a.d, w, 2_000))
        .collect();
    // A session a bot started in a folder of its own workspace.
    let nested = history(&a.d, &a.workspaces[0].join("repo"), 1_000);
    let cache = git_cache(&a);

    delete(&mut a.c, &a.id).await;
    wait_until("the project's folder is in the trash", || {
        !project_dir.exists() && trashed(&a.d, &a.dir, "project-files").join(&a.dir).is_dir()
    })
    .await;
    for dir in histories.iter().chain([&nested]) {
        assert!(!dir.exists(), "{} still there", dir.display());
        let name = dir.file_name().unwrap();
        let moved = trashed(&a.d, &a.dir, "session-history").join(name);
        assert!(
            moved.join("session.jsonl").is_file(),
            "{} not trashed",
            moved.display()
        );
    }
    assert!(!cache.exists());
    let moved = trashed(&a.d, &a.dir, "git-cache").join(format!("{}.git", a.id));
    assert!(moved.join("HEAD").is_file());
    // The bots' folders went with it, so nothing was erased.
    let ws = trashed(&a.d, &a.dir, "project-files").join(format!("{}/bots", a.dir));
    assert_eq!(std::fs::read_dir(ws).unwrap().count(), 2);
}

#[tokio::test]
async fn the_preview_lists_what_will_move_with_sizes() {
    let mut a = acme().await;
    for w in &a.workspaces {
        history(&a.d, w, 2_000);
    }
    git_cache(&a);
    let project_dir = a.d.app.cfg.projects_dir().join(&a.dir);
    let folder = flotillad::trash::size_of(&project_dir);

    let p = preview(&mut a.c, &a.id).await;
    assert_eq!(group(&p, "project_files")["bytes"], folder);
    assert_eq!(group(&p, "session_history")["bytes"], 4_000);
    assert_eq!(group(&p, "session_history")["count"], 2);
    assert_eq!(group(&p, "git_cache")["bytes"], 3_000);
    assert_eq!(p["total_bytes"], folder + 7_000);
    assert_eq!(p["bin"], "trash folder");
    assert_eq!(p["peers"], json!([]));
    assert!(project_dir.exists(), "a preview moves nothing");

    // Owner only: never on the owner token a bot can read.
    let mut token = WsClient::connect_owner_token(&a.d).await;
    let refused = token
        .request(json!({ "type": "project_delete_preview", "project_id": a.id }))
        .await;
    assert_eq!(refused["code"], "forbidden", "{refused}");
}

/// A process with its working folder in `dir`, until killed.
fn hold(dir: &Path) -> std::process::Child {
    let mut command = if cfg!(windows) {
        let mut c = std::process::Command::new("cmd");
        c.args(["/C", "ping -n 60 127.0.0.1 >NUL"]);
        c
    } else {
        let mut c = std::process::Command::new("sleep");
        c.arg("60");
        c
    };
    command.current_dir(dir).spawn().unwrap()
}

#[tokio::test]
async fn an_item_a_running_process_holds_is_left_and_reported() {
    let mut a = acme().await;
    let held = history(&a.d, &a.workspaces[0], 500);
    let free = history(&a.d, &a.workspaces[1], 500);
    let mut holder = hold(&held);

    delete(&mut a.c, &a.id).await;
    let notice =
        a.c.wait_for(|m| {
            m["type"] == "notify"
                && m["title"]
                    .as_str()
                    .is_some_and(|t| t.contains("cleaned up"))
        })
        .await;
    holder.kill().unwrap();
    let _ = holder.wait();
    assert!(
        notice["body"].as_str().unwrap().contains("in use"),
        "{notice}"
    );
    assert!(held.join("session.jsonl").is_file(), "held, so left");
    assert!(!free.exists(), "the rest still moved");
}

#[tokio::test]
async fn cleaning_up_deleted_projects_trashes_what_they_left() {
    let mut a = acme().await;
    let histories: Vec<PathBuf> = a
        .workspaces
        .iter()
        .map(|w| history(&a.d, w, 1_000))
        .collect();
    let cache = git_cache(&a);
    // Deleted as before 0.19: the rows archived, every folder kept.
    let project = a.d.app.db.get_project(&a.id).unwrap().unwrap();
    flotillad::projectmgmt::archive_project(&a.d.app, &project, &flotillad::db::Actor::User)
        .unwrap();

    let reply =
        a.c.request(json!({ "type": "deleted_projects_preview" }))
            .await;
    assert_eq!(reply["type"], "deleted_projects_preview", "{reply}");
    let found = &reply["preview"]["projects"][0];
    assert_eq!(found["project_name"], "Acme", "{reply}");
    assert_eq!(group(found, "session_history")["bytes"], 2_000);
    assert_eq!(group(found, "git_cache")["bytes"], 3_000);

    let done =
        a.c.request(json!({ "type": "deleted_projects_cleanup" }))
            .await;
    assert_eq!(done["type"], "deleted_projects_cleaned", "{done}");
    assert!(
        done["summary"].as_str().unwrap().starts_with("Moved"),
        "{done}"
    );
    for dir in &histories {
        assert!(!dir.exists());
    }
    assert!(!cache.exists());
    assert!(!a.d.app.cfg.projects_dir().join(&a.dir).exists());
    assert!(trashed(&a.d, &a.dir, "git-cache")
        .join(format!("{}.git", a.id))
        .exists());

    let again =
        a.c.request(json!({ "type": "deleted_projects_preview" }))
            .await;
    assert_eq!(again["preview"]["projects"], json!([]), "nothing left");
}
