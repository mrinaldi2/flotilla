//! Lighter checkouts (0.23.0, `repo_mirror_v1`): a card's worktree comes
//! from the project's mirror through `worktree_for`, one per card, sparse
//! when asked; it goes once the card's PR merges, or is salvaged and goes
//! when the card is cancelled. The read-only repo tools read the mirror,
//! and a project over its disk budget loses merged cards' worktrees first.

use crate::common;

use std::path::{Path, PathBuf};

use chrono::Utc;
use common::cleanup::{jobs, step};
use common::prs::{error, head, setup, Repo};
use common::repo::git;
use flotillad::actor::Actor;
use flotillad::db::MoveTo;
use serde_json::{json, Value};

const DEV: usize = 1;
const ARCH: usize = 2;

/// Puts `docs/guide.md` and `src/lib.rs` on the remote's main.
fn grow(r: &Repo) {
    let seed = r.dev.join("grow");
    let origin = r.origin.display().to_string();
    git(&r.dev, &["clone", "-q", &origin, "grow"]);
    std::fs::create_dir_all(seed.join("docs")).unwrap();
    std::fs::create_dir_all(seed.join("src")).unwrap();
    std::fs::write(seed.join("docs/guide.md"), "# The guide\n").unwrap();
    std::fs::write(seed.join("src/lib.rs"), "pub fn search() {}\n").unwrap();
    git(&seed, &["add", "."]);
    git(&seed, &["commit", "-q", "-m", "grow"]);
    git(&seed, &["push", "-q", "origin", "main"]);
}

async fn tree_for(r: &mut Repo, bot: usize, args: Value) -> (Value, PathBuf) {
    let out = r.bots[bot].call("worktree_for", args).await;
    let path = PathBuf::from(out["path"].as_str().expect("path"));
    (out, path)
}

/// One commit in `tree`, pushed to its branch.
fn work(tree: &Path, file: &str) -> String {
    std::fs::write(tree.join(file), "work\n").unwrap();
    git(tree, &["add", file]);
    git(tree, &["commit", "-q", "-m", file]);
    git(tree, &["push", "-q", "origin", "HEAD"]);
    head(tree)
}

fn rows(r: &Repo) -> Vec<flotillad::db::CardWorktree> {
    r.pair.d.app.db.card_worktrees(Some(&r.project)).unwrap()
}

#[tokio::test]
async fn a_card_gets_one_worktree_shared_by_its_bots_and_sparse_when_asked() {
    let mut r = setup().await;
    grow(&r);
    let card = r.card("Search", "doing");
    let (first, tree) = tree_for(&mut r, DEV, json!({"item_id": card})).await;
    assert_eq!(first["created"], true, "{first}");
    assert_eq!(first["base"], "main");
    assert!(
        first["branch"].as_str().unwrap().starts_with("flotilla/h-"),
        "{first}"
    );
    assert!(tree.join("docs/guide.md").exists() && tree.join("src/lib.rs").exists());
    let home = flotillad::safe_git::canonical(&r.pair.d.app.cfg.home).unwrap();
    assert!(
        tree.starts_with(home.join("projects")),
        "{}",
        tree.display()
    );
    assert!(tree.parent().unwrap().ends_with("worktrees"));
    assert!(
        home.join("repos").read_dir().unwrap().count() == 1,
        "one mirror"
    );

    // The Architect asking for the same card gets the same tree.
    let (again, same) = tree_for(&mut r, ARCH, json!({"item_id": card})).await;
    assert_eq!(again["created"], false);
    assert_eq!(same, tree);

    // Sparse: only the folders asked for, then widened.
    let docs = r.card("Docs", "doing");
    let (sparse, small) = tree_for(&mut r, DEV, json!({"item_id": docs, "paths": ["docs"]})).await;
    assert_eq!(sparse["paths"], json!(["docs"]));
    assert!(small.join("docs/guide.md").exists());
    assert!(!small.join("src").exists());
    let (wider, _) = tree_for(&mut r, ARCH, json!({"item_id": docs, "paths": ["src/"]})).await;
    assert_eq!(wider["paths"], json!(["docs", "src"]));
    assert!(small.join("src/lib.rs").exists());
    assert_eq!(rows(&r).len(), 2);

    // A tree removed by hand is made again.
    std::fs::remove_dir_all(&small).unwrap();
    let (remade, back) = tree_for(&mut r, DEV, json!({"item_id": docs})).await;
    assert_eq!(remade["created"], true);
    assert_eq!(back, small);

    for (args, why) in [
        (
            json!({"item_id": card, "paths": ["../up"]}),
            "isn't a folder",
        ),
        (json!({"item_id": "H-999"}), "no card H-999"),
        (
            json!({"item_id": card, "paths": "docs"}),
            "a list of folders",
        ),
    ] {
        let refused = r.bots[DEV].call_raw("worktree_for", args).await;
        assert!(error(&refused).contains(why), "{refused}");
    }
    let tools = r.bots[DEV].tools().await.to_string();
    for name in [
        "worktree_for",
        "repo_file",
        "repo_diff",
        "repo_log",
        "repo_grep",
    ] {
        assert!(tools.contains(name), "{name}");
    }
}

#[tokio::test]
async fn a_cards_worktree_goes_once_its_pr_merges_reported_or_not() {
    let mut r = setup().await;
    let a = r.card("Search", "doing");
    let (out, reported) = tree_for(&mut r, DEV, json!({"item_id": a})).await;
    let branch = out["branch"].as_str().unwrap().to_string();
    work(&reported, "search.txt");
    let opened = r.bots[DEV]
        .call(
            "pr_open",
            json!({"item": a, "branch": branch, "worktree": reported.display().to_string()}),
        )
        .await;
    let number = opened["pr"]["number"].as_u64().unwrap() as u32;
    common::cleanup::merge(&r, number, &reported);
    step(&r, Utc::now()).await;
    assert!(!reported.exists(), "the reported card tree is gone");

    // Not passed to pr_open: the cleanup finds it on the branch in the mirror.
    let b = r.card("Filter", "doing");
    let (out, found) = tree_for(&mut r, DEV, json!({"item_id": b})).await;
    let branch = out["branch"].as_str().unwrap().to_string();
    work(&found, "filter.txt");
    let opened = r.bots[DEV]
        .call("pr_open", json!({"item": b, "branch": branch}))
        .await;
    let number = opened["pr"]["number"].as_u64().unwrap() as u32;
    let pr = common::cleanup::merge(&r, number, &found);
    step(&r, Utc::now()).await;
    assert!(!found.exists(), "the discovered card tree is gone");
    let done = jobs(&r, &pr);
    assert!(
        done.iter()
            .any(|j| j.path_or_ref.ends_with(&b.to_lowercase())
                && j.bot_id.as_deref() == Some(r.pair.ids[DEV].as_str())),
        "{done:?}"
    );
    // The pass forgets rows whose tree is gone.
    flotillad::checkouts::tidy::pass(&r.pair.d.app);
    assert!(rows(&r).is_empty());
}

#[tokio::test]
async fn a_cancelled_cards_worktree_is_salvaged_then_removed() {
    let mut r = setup().await;
    let doomed = r.card("Dropped", "doing");
    let kept = r.card("Kept", "doing");
    let (_, tree) = tree_for(&mut r, DEV, json!({"item_id": doomed})).await;
    let (_, live) = tree_for(&mut r, DEV, json!({"item_id": kept})).await;
    std::fs::write(tree.join("draft.txt"), "unsaved idea\n").unwrap();
    std::fs::write(tree.join("local.txt"), "local\n").unwrap();
    git(&tree, &["add", "local.txt"]);
    git(&tree, &["commit", "-q", "-m", "unpushed"]);

    flotillad::checkouts::tidy::pass(&r.pair.d.app);
    assert!(tree.exists(), "a live card's tree stays");

    let db = &r.pair.d.app.db;
    let item = db.get_item(&doomed).unwrap().unwrap();
    let to = MoveTo {
        column: "cancelled",
        ..MoveTo::default()
    };
    db.move_item(&doomed, item.version, &to, &Actor::User)
        .unwrap();
    flotillad::checkouts::tidy::pass(&r.pair.d.app);
    assert!(!tree.exists(), "the cancelled card's tree is gone");
    assert!(live.exists(), "the other card's stays");
    let salvage = r
        .pair
        .d
        .app
        .cfg
        .home
        .join("salvage")
        .join(&r.project)
        .join(&doomed);
    let saved: Vec<String> = walk(&salvage);
    assert!(saved.iter().any(|f| f.contains("draft.txt")), "{saved:?}");
    assert!(saved.iter().any(|f| f.ends_with(".bundle")), "{saved:?}");
    assert_eq!(rows(&r).len(), 1);
}

fn walk(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(walk(&path));
        } else {
            out.push(path.display().to_string());
        }
    }
    out
}

#[tokio::test]
async fn the_repo_tools_read_the_mirror_at_any_ref() {
    let mut r = setup().await;
    grow(&r);
    // The project names a branch the repository doesn't have: its default
    // branch is read instead.
    let db = &r.pair.d.app.db;
    db.set_project_repo(
        &r.project,
        Some(&bus::ProjectRepo {
            url: r.origin.display().to_string(),
            branch: "develop".into(),
        }),
    )
    .unwrap();
    let arch = &mut r.bots[ARCH];
    let guide = arch
        .call("repo_file", json!({"path": "docs/guide.md"}))
        .await;
    assert_eq!(guide["text"], "# The guide\n");
    assert_eq!(guide["ref"], "main");
    let top = arch.call("repo_file", json!({"path": "docs"})).await;
    assert_eq!(top["entries"][0]["name"], "guide.md");
    let log = arch.call("repo_log", json!({"limit": 1})).await;
    assert_eq!(log["commits"][0]["subject"], "grow");
    let hits = arch
        .call("repo_grep", json!({"pattern": "fn search"}))
        .await;
    assert_eq!(hits["matches"][0]["path"], "src/lib.rs");

    let seed = r.dev.join("grow");
    git(&seed, &["checkout", "-q", "-b", "topic"]);
    std::fs::write(seed.join("docs/guide.md"), "# The guide\n\nMore.\n").unwrap();
    git(&seed, &["commit", "-q", "-am", "more"]);
    git(&seed, &["push", "-q", "origin", "topic"]);
    // A tree's fetch brings the new branch in.
    let card = r.card("More", "doing");
    tree_for(&mut r, DEV, json!({"item_id": card})).await;
    let arch = &mut r.bots[ARCH];
    let diff = arch
        .call("repo_diff", json!({"base": "main", "head": "topic"}))
        .await;
    assert_eq!(diff["files"][0]["path"], "docs/guide.md");
    assert!(diff["diff"].as_str().unwrap().contains("+More."));

    for (tool, args, why) in [
        (
            "repo_file",
            json!({"path": "../../etc/passwd"}),
            "isn't a path",
        ),
        (
            "repo_file",
            json!({"path": "README.md", "ref": "--upload-pack=x"}),
            "isn't a branch",
        ),
        ("repo_grep", json!({"pattern": ""}), "a pattern"),
        ("repo_diff", json!({}), "'base' is required"),
    ] {
        let refused = arch.call_raw(tool, args).await;
        assert!(error(&refused).contains(why), "{tool}: {refused}");
    }
}

#[tokio::test]
async fn the_owner_sets_a_budget_and_merged_cards_trees_go_first_when_over_it() {
    let mut r = setup().await;
    let set = |bytes: Value| json!({"type": "set_project_disk_budget", "project_id": r.project, "bytes": bytes});
    let mut token = common::WsClient::connect_owner_token(&r.pair.d).await;
    let refused = token.request(set(json!(1))).await;
    assert_eq!(refused["type"], "error", "{refused}");
    let mut app = common::WsClient::connect(&r.pair.d).await;
    for bad in [json!(0), json!(-5), json!("big")] {
        let answer = app.request(set(bad)).await;
        assert_eq!(answer["type"], "error", "{answer}");
    }
    let cleared = app.request(set(Value::Null)).await;
    assert!(
        cleared["project"]["disk_budget_bytes"].is_null(),
        "{cleared}"
    );
    let budget = app.request(set(json!(1))).await;
    assert_eq!(budget["project"]["disk_budget_bytes"], 1, "{budget}");

    // One card merged, one still in progress.
    let merged = r.card("Merged", "doing");
    let (out, done) = tree_for(&mut r, DEV, json!({"item_id": merged})).await;
    work(&done, "merged.txt");
    let opened = r.bots[DEV]
        .call("pr_open", json!({"item": merged, "branch": out["branch"]}))
        .await;
    let number = opened["pr"]["number"].as_u64().unwrap() as u32;
    let sha = head(&done);
    git(&done, &["push", "-q", "origin", "HEAD:main"]);
    let db = &r.pair.d.app.db;
    let open = common::cleanup::pr(&r, number);
    db.board_tx(|t| t.set_pr_merged(&open, &sha, &r.pair.ids[0]))
        .unwrap();
    let live = r.card("Live", "doing");
    let (_, busy) = tree_for(&mut r, DEV, json!({"item_id": live})).await;

    let report = flotillad::checkouts::budget::hourly(&r.pair.d.app).unwrap();
    assert!(!done.exists(), "the merged card's tree went first");
    assert!(
        busy.exists(),
        "a live card's tree is never removed for the budget"
    );
    let over = flotillad::checkouts::budget::over(&report);
    assert_eq!(over.len(), 1, "{report}");
    assert_eq!(over[0].0, r.project);

    let rows = app
        .request(json!({"type": "attention_rows", "project_id": r.project}))
        .await;
    let titles: Vec<String> = rows["attention_rows"]["rows"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .map(|r| r["title"].as_str().unwrap_or_default().to_string())
        .collect();
    assert!(
        titles
            .iter()
            .any(|t| t.starts_with("Over its disk budget on ")),
        "{rows}"
    );
}
