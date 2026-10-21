//! A PR approved where nobody holds DevOps (0.23.3): after the Undo window
//! it waits, the owner is told once on Needs you and on the card, and it is
//! handed to DevOps as soon as a bot holds the role.

use crate::common;

use chrono::{Duration, Utc};
use common::prs::{approve, give, opened, pr, setup, Repo};
use common::WsClient;
use flotillad::kanban::model::Role;
use flotillad::prs::queue;
use serde_json::{json, Value};

async fn owner_approve(r: &Repo, sha: &str) {
    let mut app = WsClient::connect(&r.pair.d).await;
    app.request(
        json!({"type": "pr_review_submit", "project_id": r.project, "number": 1,
                       "sha": sha, "verdict": "approved"}),
    )
    .await;
}

async fn stuck_rows(r: &Repo) -> Vec<Value> {
    let mut app = WsClient::connect(&r.pair.d).await;
    let rows = app
        .request(json!({"type": "attention_rows", "project_id": r.project}))
        .await;
    rows["attention_rows"]["rows"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|row| row["kind"] == "PR_MERGE_STUCK")
        .collect()
}

async fn daemon_comments(r: &mut Repo, item: &str) -> Vec<String> {
    let got = r.bots[0].call("item_get", json!({"id": item})).await;
    got["item"]["comments"]
        .as_array()
        .or_else(|| got["comments"].as_array())
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|c| c["body"].as_str().map(str::to_string))
        .filter(|b| b.contains("DevOps"))
        .collect()
}

#[tokio::test]
async fn an_approved_pr_with_no_devops_tells_the_owner_once_then_merges_when_one_is_given() {
    let mut r = setup().await;
    let (_, head) = opened(&mut r).await;
    approve(&mut r, &head).await;
    owner_approve(&r, &head).await;
    let app = r.pair.d.app.clone();
    let item = pr(&mut r).await["item_id"].as_str().unwrap().to_string();
    // The window opened 20 s ago and ended 10 s ago, with nobody in DevOps.
    let now = Utc::now();
    queue::step(&app, now - Duration::seconds(20)).unwrap();
    queue::step(&app, now).unwrap();
    queue::step(&app, now + Duration::seconds(2)).unwrap();

    let waiting = pr(&mut r).await;
    assert_eq!(waiting["state"], "merging", "{waiting}");
    assert_eq!(waiting["merge"]["state"], "window");
    assert_eq!(waiting["merge"]["waiting"], "no_devops", "{waiting}");
    let rows = stuck_rows(&r).await;
    assert_eq!(rows.len(), 1, "{rows:?}");
    let title = rows[0]["title"].as_str().unwrap();
    assert!(title.contains("nobody holds DevOps"), "{title}");
    assert!(title.contains("into main"), "{title}");
    assert_eq!(
        daemon_comments(&mut r, &item).await.len(),
        1,
        "the card is told once, not every tick"
    );

    give(&r, 0, Role::Devops);
    queue::step(&app, now + Duration::seconds(4)).unwrap();
    let handed = pr(&mut r).await;
    assert_eq!(handed["merge"]["state"], "handed", "{handed}");
    assert_eq!(app.db.open_tasks_for(&r.pair.ids[0]).unwrap().len(), 1);
    assert!(stuck_rows(&r).await.is_empty(), "the wait is over");
}
