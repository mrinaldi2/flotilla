//! What a project delete may trash (0.19.0): never anything outside the
//! places a project leaves things, whatever a bot's stored path or a
//! planted link says; and a bot's worktree beside the owner's clones only
//! when no other project's bot could own it.

use crate::common;

use common::create_bot;
use common::peers::wait_until;
use common::trash::{
    acme, acme_with, craft_workspace, delete, group, history, key, preview, trashed,
};
use serde_json::json;

#[tokio::test]
async fn a_bot_path_crafted_to_point_elsewhere_is_never_trashed() {
    let mut a = acme().await;
    let mine = a.d.app.cfg.user_home.join("Developer/mine");
    std::fs::create_dir_all(&mine).unwrap();
    std::fs::write(mine.join("work.txt"), "the owner's").unwrap();
    let owners_history = history(&a.d, &mine, 500);
    let bot = create_bot(&mut a.c, &a.id, "evil").await;
    craft_workspace(&a.d, bot["id"].as_str().unwrap(), &mine);

    let p = preview(&mut a.c, &a.id).await;
    let refused = p["refused"].to_string();
    assert!(refused.contains("not in its project's folder"), "{p}");
    assert!(!p.to_string().contains(&key(&mine)), "{p}");

    delete(&mut a.c, &a.id).await;
    let project_dir = a.d.app.cfg.projects_dir().join(&a.dir);
    wait_until("the project is trashed", || !project_dir.exists()).await;
    assert!(
        owners_history.join("session.jsonl").is_file(),
        "the owner's history stays"
    );
    assert_eq!(
        std::fs::read_to_string(mine.join("work.txt")).unwrap(),
        "the owner's"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_link_never_leads_the_trash_outside_its_root() {
    use common::trash::claude;
    let mut a = acme().await;
    let outside = a.d.app.cfg.user_home.join("elsewhere");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("keep.txt"), "kept").unwrap();
    // The first bot's history folder is a link out of ~/.claude/projects.
    std::fs::create_dir_all(claude(&a.d)).unwrap();
    let link = claude(&a.d).join(key(&a.workspaces[0]));
    std::os::unix::fs::symlink(&outside, &link).unwrap();
    // So is the git cache.
    let repos = a.d.app.cfg.home.join("cache/repos");
    std::fs::create_dir_all(&repos).unwrap();
    let cache = repos.join(format!("{}.git", a.id));
    std::os::unix::fs::symlink(&outside, &cache).unwrap();

    let p = preview(&mut a.c, &a.id).await;
    let refused = p["refused"].to_string();
    assert!(refused.contains("link"), "{p}");
    assert!(
        p["groups"]
            .as_array()
            .unwrap()
            .iter()
            .all(|g| g["group"] != "git_cache"),
        "{p}"
    );

    delete(&mut a.c, &a.id).await;
    let project_dir = a.d.app.cfg.projects_dir().join(&a.dir);
    wait_until("the project is trashed", || !project_dir.exists()).await;
    assert!(std::fs::symlink_metadata(&link).is_ok(), "the link stays");
    assert!(std::fs::symlink_metadata(&cache).is_ok(), "the link stays");
    assert_eq!(
        std::fs::read_to_string(outside.join("keep.txt")).unwrap(),
        "kept"
    );
}

#[tokio::test]
async fn a_bots_worktree_beside_the_owners_clones_is_trashed_and_pruned() {
    use common::repo::{git, remote};
    // The owner's clones are outside the daemon's home, as on a real
    // computer: the test home holds the user's home otherwise.
    let user = tempfile::tempdir().unwrap();
    let user_home = user.path().to_path_buf();
    let mut a = acme_with(|cfg| cfg.user_home = user_home).await;
    let origins = tempfile::tempdir().unwrap();
    let url = remote(origins.path()).display().to_string();
    let repo = bus::ProjectRepo {
        url: url.clone(),
        branch: "main".into(),
    };
    a.d.app.db.set_project_repo(&a.id, Some(&repo)).unwrap();
    // Another project has an `ops` too: a tree named for `ops` could be its.
    let other =
        a.c.request(json!({ "type": "create_project", "name": "Other" }))
            .await;
    create_bot(&mut a.c, other["project"]["id"].as_str().unwrap(), "ops").await;

    let developer = a.d.app.cfg.user_home.join("Developer");
    std::fs::create_dir_all(&developer).unwrap();
    git(&developer, &["clone", "-q", &url, "gravity"]);
    let main = developer.join("gravity");
    let tree = |name: &str, branch: &str| {
        let path = developer.join(name);
        git(
            &main,
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                branch,
                &path.display().to_string(),
            ],
        );
        std::fs::write(path.join("notes.txt"), "uncommitted, kept in the trash\n").unwrap();
        path
    };
    let devs = tree("gravity-wt-dev", "H-1-dev");
    let ops = tree("gravity-wt-ops", "H-2-ops");

    let p = preview(&mut a.c, &a.id).await;
    assert_eq!(group(&p, "worktrees")["count"], 1, "{p}");
    assert!(p["refused"].to_string().contains("could own it too"), "{p}");

    delete(&mut a.c, &a.id).await;
    let notice =
        a.c.wait_for(|m| {
            m["type"] == "notify"
                && m["title"]
                    .as_str()
                    .is_some_and(|t| t.contains("cleaned up"))
        })
        .await;
    assert!(!devs.exists(), "{notice}");
    let moved = trashed(&a.d, &a.dir, "worktrees").join("gravity-wt-dev");
    assert!(
        moved.join("notes.txt").is_file(),
        "its uncommitted work is in the trash"
    );
    assert!(
        ops.join("notes.txt").is_file(),
        "another project's bot could own it"
    );
    wait_until("the main clone forgets it", || {
        !git(&main, &["worktree", "list"]).contains("gravity-wt-dev")
    })
    .await;
    assert!(main.join(".git").is_dir(), "the owner's clone stays");
}
