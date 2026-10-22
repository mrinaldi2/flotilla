//! Changes a bot reviewer asks for (0.23.5): the bot that opened the PR is
//! tasked with them, so a request never waits on nobody.

use crate::common;

use common::prs::{give, opened, setup};
use flotillad::kanban::model::Role;
use serde_json::json;

#[tokio::test]
async fn a_reviewers_must_fix_tasks_the_prs_author() {
    let mut r = setup().await;
    let (_tree, head) = opened(&mut r).await;
    give(&r, 2, Role::ReviewerArch);
    r.bots[2]
        .call(
            "pr_review",
            json!({"number": 1, "sha": head, "role": "architect",
                   "verdict": "changes_requested", "summary": "The prints lie.",
                   "findings": [{"severity": "must", "text": "Print the real exit code"}]}),
        )
        .await;
    let db = &r.pair.d.app.db;
    let asks: Vec<String> = db
        .open_tasks_for(&r.pair.ids[1])
        .unwrap()
        .iter()
        .filter_map(|t| db.get_message(&t.origin_message_id).unwrap())
        .map(|m| m.body)
        .filter(|b| b.contains("asked for changes on PR #1"))
        .collect();
    assert_eq!(asks.len(), 1, "{asks:?}");
    assert!(asks[0].contains("(as architect)"), "{}", asks[0]);
    assert!(asks[0].contains("The prints lie."), "{}", asks[0]);
}

/// 0.24.0: the fix is pushed and the reviewer approves it, so its finding
/// shows as resolved in that commit, not "0 of 1 resolved".
#[tokio::test]
async fn approving_the_fix_resolves_the_finding() {
    let mut r = setup().await;
    let (tree, head) = opened(&mut r).await;
    give(&r, 2, Role::ReviewerArch);
    r.bots[2]
        .call(
            "pr_review",
            json!({"number": 1, "sha": head, "role": "architect",
                   "verdict": "changes_requested", "summary": "One to fix.",
                   "findings": [{"severity": "should", "text": "Log it too"}]}),
        )
        .await;
    common::prs::write(&tree, "fix.txt", "logged\n");
    common::repo::git(&tree, &["push", "-q", "origin", "H-1-search"]);
    let fixed = common::prs::report(&mut r, &tree).await;
    r.bots[2]
        .call(
            "pr_review",
            json!({"number": 1, "sha": fixed, "role": "architect", "verdict": "approved"}),
        )
        .await;
    let got = common::prs::pr(&mut r).await;
    let finding = &got["reviews"][0]["findings"][0];
    assert_eq!(finding["resolved_in"], fixed.as_str(), "{got}");
}
