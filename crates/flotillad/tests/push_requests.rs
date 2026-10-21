//! Push requests (0.24.0, `push_requests_v1`): a bot asks with
//! `request_push`, the owner taps Push, and exactly `git push origin
//! <branch>:<target>` runs in the bot's worktree. The project's branch and
//! `main` are refused, and a branch that moved since the request fails.

use crate::common;

use std::path::PathBuf;

use common::peers::wait_until;
use common::prs::{clone, commit, error, setup, Repo};
use common::tasks::drain_until;
use common::*;
use serde_json::{json, Value};

const FEATURES: &[&str] = &["permission_cards", "terminal_card", "owner_actions"];

/// Desktop Dev's worktree on `branch`, one commit ahead and not pushed.
fn unpushed(r: &Repo, branch: &str) -> PathBuf {
    let path = r.dev.join("gravity-wt-desktopdev-push");
    clone(&r.origin, &path, branch);
    commit(&path, "notes.txt", "one\ntwo\nthree\n");
    path
}

async fn ask(r: &mut Repo, args: Value) -> Value {
    r.bots[1].call_raw("request_push", args).await
}

fn action(raw: &Value) -> Value {
    assert_ne!(raw["isError"], json!(true), "refused: {raw}");
    let text = raw["content"][0]["text"].as_str().expect("text");
    serde_json::from_str::<Value>(text).expect("json")["owner_action"].clone()
}

/// The owner's app: owner actions shown, approve held, on a device.
async fn owner_app(r: &mut Repo) -> WsClient {
    let mut owner = WsClient::connect(&r.pair.d).await;
    let created = owner
        .request(json!({"type": "create_device", "name": "app",
                        "capabilities": ["read", "control", "approve"]}))
        .await;
    WsClient::connect_with(&r.pair.d, token_str(&created), FEATURES).await
}

async fn finished(r: &Repo, id: &str) -> Value {
    let db = &r.pair.d.app.db;
    wait_until("the push finishes", || {
        db.get_owner_action(id)
            .unwrap()
            .is_some_and(|a| !matches!(a.state.as_str(), "proposed" | "running"))
    })
    .await;
    db.get_owner_action(id).unwrap().unwrap().to_json()
}

fn remote_head(r: &Repo, branch: &str) -> Option<String> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(&r.origin)
        .args([
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("refs/heads/{branch}"),
        ])
        .output()
        .unwrap();
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

#[tokio::test]
async fn a_bot_asks_and_the_owner_pushes_exactly_that_branch() {
    let mut r = setup().await;
    let tree = unpushed(&r, "feature/notes");
    let head = common::prs::head(&tree);
    let card = r.card("Notes", "doing");
    let tools = r.bots[1].tools().await.to_string();
    assert!(tools.contains("request_push"), "the bots' tool list has it");

    let asked = action(
        &ask(
            &mut r,
            json!({"branch": "feature/notes", "item": card, "reason": "my pushes are blocked",
                   "worktree": tree.display().to_string()}),
        )
        .await,
    );
    assert_eq!(asked["kind"], "push", "{asked}");
    assert_eq!(asked["state"], "proposed");
    assert_eq!(
        asked["content"],
        "git push origin feature/notes:feature/notes"
    );
    assert_eq!(asked["item_id"], card.as_str());
    let push = &asked["push"];
    assert_eq!(push["branch"], "feature/notes");
    assert_eq!(push["target"], "feature/notes");
    assert_eq!(push["commit"], head.as_str());
    assert_eq!(push["item_id"], card.as_str());
    assert_eq!(
        (push["files"].clone(), push["additions"].clone()),
        (json!(1), json!(3))
    );
    assert_eq!(push["deletions"], 0);
    assert!(!push["repo"].as_str().unwrap().is_empty());
    let cwd = asked["cwd"].as_str().unwrap();
    assert_eq!(
        flotillad::safe_git::canonical(std::path::Path::new(cwd)).unwrap(),
        flotillad::safe_git::canonical(&tree).unwrap()
    );
    // Without a worktree named, the card's worktree on the branch would be
    // used; the bot has none here.
    let none = ask(&mut r, json!({"branch": "feature/notes", "reason": "r"})).await;
    assert!(error(&none).contains("no worktree"), "{none}");

    // The owner runs it from the app; the branch lands on the remote.
    let mut app = owner_app(&mut r).await;
    let id = asked["id"].as_str().unwrap();
    let run = json!({"type": "owner_action_run", "id": id, "sha256": asked["sha256"]});
    let started = app.request(run).await;
    assert_eq!(started["action"]["kind"], "push", "{started}");
    let done = finished(&r, id).await;
    assert_eq!(done["state"], "succeeded", "{done}");
    assert_eq!(
        remote_head(&r, "feature/notes").as_deref(),
        Some(head.as_str())
    );
    let notes = drain_until(&mut r.bots[1], "succeeded").await;
    assert!(!notes.is_empty());

    // Listed with its kind, as every owner action is.
    let listed = app.request(json!({"type": "owner_action_list"})).await;
    let row = listed["actions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["id"] == id)
        .cloned()
        .unwrap();
    assert_eq!(row["kind"], "push");
}

#[tokio::test]
async fn the_projects_branch_and_main_are_releases_not_pushes() {
    let mut r = setup().await;
    let db = &r.pair.d.app.db;
    db.set_project_repo(
        &r.project,
        Some(&bus::ProjectRepo {
            url: r.origin.display().to_string(),
            branch: "develop".into(),
        }),
    )
    .unwrap();
    let tree = unpushed(&r, "feature/notes");
    let at = tree.display().to_string();
    for target in ["main", "develop"] {
        let raw = ask(
            &mut r,
            json!({"branch": "feature/notes", "target": target, "reason": "ship it",
                   "worktree": at}),
        )
        .await;
        assert!(
            error(&raw).contains("is the project's branch"),
            "{target}: {raw}"
        );
    }
    let raw = ask(
        &mut r,
        json!({"branch": "-f", "reason": "r", "worktree": at}),
    )
    .await;
    assert!(error(&raw).contains("isn't a branch name"), "{raw}");
    let raw = ask(
        &mut r,
        json!({"branch": "feature/notes", "reason": " ", "worktree": at}),
    )
    .await;
    assert!(error(&raw).contains("reason"), "{raw}");
    // Another target is fine.
    let asked = action(
        &ask(
            &mut r,
            json!({"branch": "feature/notes", "target": "review/notes", "reason": "r",
                   "worktree": at}),
        )
        .await,
    );
    assert_eq!(
        asked["content"],
        "git push origin feature/notes:review/notes"
    );
}

#[tokio::test]
async fn a_branch_that_moved_since_the_request_is_not_pushed() {
    let mut r = setup().await;
    let tree = unpushed(&r, "feature/notes");
    let asked = action(
        &ask(
            &mut r,
            json!({"branch": "feature/notes", "reason": "r",
                   "worktree": tree.display().to_string()}),
        )
        .await,
    );
    commit(&tree, "more.txt", "later\n");
    let mut app = owner_app(&mut r).await;
    let id = asked["id"].as_str().unwrap();
    app.request(json!({"type": "owner_action_run", "id": id, "sha256": asked["sha256"]}))
        .await;
    let done = finished(&r, id).await;
    assert_eq!(done["state"], "failed", "{done}");
    assert!(
        done["output_tail"]
            .as_str()
            .unwrap()
            .contains("moved since the push was requested"),
        "{done}"
    );
    assert_eq!(remote_head(&r, "feature/notes"), None, "nothing was pushed");
}
