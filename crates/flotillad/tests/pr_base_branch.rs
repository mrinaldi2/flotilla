//! A project whose PRs land on another branch than main (0.23.2): the
//! owner's repository branch is the PRs' base, and a remote with no main
//! at all still opens them.

use crate::common;

use common::prs::{clone, commit, error, head, setup};
use common::repo::git;
use serde_json::json;

#[tokio::test]
async fn prs_land_on_the_projects_branch_even_with_no_main() {
    let mut r = setup().await;
    // develop moves past main, then main goes: the remote has only develop.
    let seed = r.dev.join("seed-develop");
    clone(&r.origin, &seed, "develop");
    let develop = commit(&seed, "develop.txt", "on develop\n");
    git(&seed, &["push", "-q", "origin", "develop"]);
    git(&r.origin, &["symbolic-ref", "HEAD", "refs/heads/develop"]);
    git(&r.origin, &["branch", "-q", "-D", "main"]);
    r.pair
        .d
        .app
        .db
        .set_project_repo(
            &r.project,
            Some(&bus::ProjectRepo {
                url: r.origin.display().to_string(),
                branch: "develop".into(),
            }),
        )
        .unwrap();

    let item = r.card("Search", "doing");
    let tree = r.worktree("a", "H-1-search");
    let pr = r.bots[1]
        .call("pr_open", json!({"item": item, "branch": "H-1-search"}))
        .await["pr"]
        .clone();
    assert_eq!(pr["head_sha"], json!(head(&tree)));
    assert_eq!(
        pr["base_sha"],
        json!(develop),
        "the PR's base is develop's tip"
    );
    assert_eq!(pr["base"], "develop", "and it says so");

    // develop moves on: the PR needs an update with develop, by name.
    commit(&seed, "later.txt", "later\n");
    git(&seed, &["push", "-q", "origin", "develop"]);
    r.bots[1]
        .call("pr_push", json!({"number": 1, "sha": head(&tree)}))
        .await;
    let got = r.bots[0].call("pr_get", json!({"number": 1})).await["pr"].clone();
    let texts: Vec<String> = got["mergeable"]["blockers"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|b| b["text"].as_str().map(str::to_string))
        .collect();
    assert!(
        texts.iter().any(|t| t == "Needs update with develop"),
        "{texts:?}"
    );

    let other = r.card("Other", "doing");
    let raw = r.bots[1]
        .call_raw("pr_open", json!({"item": other, "branch": "develop"}))
        .await;
    assert!(
        error(&raw).contains("the branch"),
        "a PR from the base branch itself is refused: {raw}"
    );
}
