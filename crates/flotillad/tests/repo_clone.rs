//! `clone_project_repo` (0.20.0, `project_repo_clone`): the owner's clone of
//! a project's repository into a folder in their home, with its progress
//! pushed to every client.

use crate::common;

use common::*;
use serde_json::{json, Value};

/// The final `repo_clone` push for the project: `done` or `failed`.
async fn finished(c: &mut WsClient, project_id: &str) -> Value {
    c.wait_for(|v| {
        v["type"] == "repo_clone"
            && v["project_id"] == project_id
            && (v["state"] == "done" || v["state"] == "failed")
    })
    .await
}

fn clone_req(project_id: &str, url: &str, dest: &str) -> Value {
    json!({"type": "clone_project_repo", "project_id": project_id, "url": url, "dest": dest})
}

#[tokio::test]
async fn the_owner_clones_into_a_folder_in_their_home() {
    let remote = tempfile::tempdir().unwrap();
    let origin = common::repo::remote(remote.path()).display().to_string();
    let d = spawn_daemon().await;
    let mut owner = WsClient::connect(&d).await;
    let pid = common::peers::project(&mut owner, "Cloned").await;

    let started = owner.request(clone_req(&pid, &origin, "~/code/app")).await;
    assert_eq!(started["type"], "repo_clone_started", "{started}");
    assert_eq!(started["project_id"], pid.as_str());
    let done = finished(&mut owner, &pid).await;
    assert_eq!(done["state"], "done", "{done}");
    let path = std::path::PathBuf::from(done["path"].as_str().unwrap());
    let home = flotillad::safe_git::canonical(&d.app.cfg.user_home).unwrap();
    assert_eq!(path, home.join("code/app"));
    assert!(path.join("README.md").is_file());
    // The project's repository is the owner's next call, not this one's.
    assert!(d.app.db.project_repo(&pid).unwrap().is_none());

    // Into a folder that now has something in it: refused.
    let full = owner.request(clone_req(&pid, &origin, "~/code/app")).await;
    assert_eq!(full["code"], "invalid_request", "{full}");
}

#[tokio::test]
async fn a_failed_clone_says_why_and_leaves_nothing() {
    let remote = tempfile::tempdir().unwrap();
    let origin = common::repo::remote(remote.path()).display().to_string();
    let d = spawn_daemon().await;
    let mut owner = WsClient::connect(&d).await;
    let pid = common::peers::project(&mut owner, "Failing").await;
    let mut req = clone_req(&pid, &origin, "~/code/app");
    req["branch"] = json!("no-such-branch");
    let started = owner.request(req).await;
    assert_eq!(started["type"], "repo_clone_started", "{started}");
    let failed = finished(&mut owner, &pid).await;
    assert_eq!(failed["state"], "failed", "{failed}");
    let error = failed["error"].as_str().unwrap();
    assert!(error.starts_with("Couldn't clone"), "{error}");
    assert!(!d.app.cfg.user_home.join("code").exists());

    // The slot is free again.
    let again = owner.request(clone_req(&pid, &origin, "~/app")).await;
    assert_eq!(again["type"], "repo_clone_started", "{again}");
    assert_eq!(finished(&mut owner, &pid).await["state"], "done");
}

#[tokio::test]
async fn bad_clones_are_refused() {
    let d = spawn_daemon().await;
    let mut owner = WsClient::connect(&d).await;
    let pid = common::peers::project(&mut owner, "Refused").await;
    let outside = tempfile::tempdir().unwrap();
    let outside = outside.path().join("app").display().to_string();
    for (req, code) in [
        (
            clone_req(&pid, "git@github.com:me/app.git", "~/app"),
            "invalid_request",
        ),
        (
            clone_req(&pid, "--upload-pack=x", "~/app"),
            "invalid_request",
        ),
        (
            clone_req(&pid, "https://example.com/app.git", &outside),
            "invalid_request",
        ),
        (
            clone_req(&pid, "https://example.com/app.git", "~"),
            "invalid_request",
        ),
        (
            clone_req(&pid, "https://example.com/app.git", "app"),
            "invalid_request",
        ),
        (
            clone_req("no-project", "https://example.com/app.git", "~/app"),
            "not_found",
        ),
    ] {
        let refused = owner.request(req).await;
        assert_eq!(refused["code"], code, "{refused}");
    }

    // One clone per project at a time.
    assert!(d.app.repo_clones.begin(&pid));
    let busy = owner
        .request(clone_req(&pid, "https://example.com/app.git", "~/app"))
        .await;
    assert_eq!(busy["code"], "conflict", "{busy}");

    // Never on the owner token a bot could read.
    let mut token = WsClient::connect_owner_token(&d).await;
    let refused = token
        .request(clone_req(&pid, "https://example.com/app.git", "~/app"))
        .await;
    assert_eq!(refused["code"], "forbidden", "{refused}");
}
