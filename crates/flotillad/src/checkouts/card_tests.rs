use std::path::{Path, PathBuf};
use std::process::Command;

use super::*;

/// git as a bot or the remote's owner would run it.
pub(crate) fn g(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .current_dir(dir)
        .args(["-c", "user.name=t", "-c", "user.email=t@t"])
        .args(args)
        .output()
        .expect("git");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// A bare remote in `dir/origin.git` whose only branch is `branch`:
/// `README`, `a/x/f` and `b/g`. Its work clone is `dir/seed`.
pub(crate) fn origin(dir: &Path, branch: &str) -> PathBuf {
    let seed = dir.join("seed");
    std::fs::create_dir_all(seed.join("a/x")).unwrap();
    std::fs::create_dir_all(seed.join("b")).unwrap();
    g(&seed, &["init", "-q", "-b", branch]);
    std::fs::write(seed.join("README"), "readme\nsecond line\n").unwrap();
    std::fs::write(seed.join("a/x/f"), "alpha fn\n").unwrap();
    std::fs::write(seed.join("b/g"), "beta\n").unwrap();
    g(&seed, &["add", "."]);
    g(&seed, &["commit", "-q", "-m", "start"]);
    g(dir, &["clone", "-q", "--bare", "seed", "origin.git"]);
    g(&seed, &["remote", "add", "origin", "../origin.git"]);
    dir.join("origin.git")
}

/// The project's mirror of `origin`, set to `branch`, not made yet.
pub(crate) fn mirror(dir: &Path, origin: &Path, branch: &str) -> Mirror {
    Mirror {
        project_id: "p".into(),
        project_dir: "p".into(),
        path: dir.join("home/repos/p.git"),
        url: origin.display().to_string(),
        branch: branch.into(),
    }
}

fn tmp() -> (tempfile::TempDir, PathBuf) {
    let t = tempfile::tempdir().unwrap();
    let dir = crate::safe_git::canonical(t.path()).unwrap();
    (t, dir)
}

#[test]
fn the_mirror_tracks_the_remote_and_falls_back_to_its_default_branch() {
    let (_t, dir) = tmp();
    let origin = origin(&dir, "develop");
    // The project says main; the repository only has develop.
    let m = mirror(&dir, &origin, "main");
    assert!(!m.exists());
    m.fetch().unwrap();
    assert!(m.exists());
    assert_eq!(m.base().unwrap(), "develop");
    assert!(m.resolve("refs/remotes/origin/develop").is_some());
    assert_eq!(
        g(&m.path, &["for-each-ref", "refs/heads"]),
        "",
        "the remote's branches are tracking refs, never local ones"
    );
    // The project's branch wins once the remote has it.
    let seed = dir.join("seed");
    g(&seed, &["push", "-q", "origin", "develop:main"]);
    g(&seed, &["commit", "-q", "--allow-empty", "-m", "next"]);
    g(&seed, &["push", "-q", "origin", "develop"]);
    m.fetch().unwrap();
    assert_eq!(m.base().unwrap(), "main");
    assert_eq!(
        m.resolve("refs/remotes/origin/develop"),
        Some(g(&seed, &["rev-parse", "HEAD"]))
    );
    // Neither, and no default: said so.
    let lost = Mirror {
        branch: "nope".into(),
        ..m.clone()
    };
    g(
        &m.path,
        &["symbolic-ref", "--delete", "refs/remotes/origin/HEAD"],
    );
    assert!(lost
        .base()
        .unwrap_err()
        .to_string()
        .contains("no branch nope"));
}

#[test]
fn a_card_worktree_starts_from_base_and_its_branch_follows_the_remote() {
    let (_t, dir) = tmp();
    let origin = origin(&dir, "main");
    let m = mirror(&dir, &origin, "main");
    m.fetch().unwrap();
    let tree = dir.join("home/projects/p/worktrees/h-1");
    add(&m, &tree, "flotilla/h-1-search", "main", &[]).unwrap();
    assert!(is_worktree(&tree));
    assert!(tree.join("b/g").exists() && tree.join("a/x/f").exists());
    assert_eq!(
        g(&tree, &["branch", "--show-current"]),
        "flotilla/h-1-search"
    );
    // git push and pull know where the branch goes.
    assert_eq!(
        g(&tree, &["config", "branch.flotilla/h-1-search.remote"]),
        "origin"
    );
    assert_eq!(
        g(&tree, &["config", "branch.flotilla/h-1-search.merge"]),
        "refs/heads/flotilla/h-1-search"
    );
    // The bot commits and pushes from the tree; a fetch then never refuses
    // because the branch is checked out.
    std::fs::write(tree.join("new.txt"), "work\n").unwrap();
    g(&tree, &["add", "new.txt"]);
    g(&tree, &["commit", "-q", "-m", "work"]);
    g(&tree, &["push", "-q", "origin", "flotilla/h-1-search"]);
    m.fetch().unwrap();
    assert_eq!(
        m.resolve("refs/remotes/origin/flotilla/h-1-search"),
        Some(g(&tree, &["rev-parse", "HEAD"]))
    );

    // A card whose branch is already on the remote starts from it.
    let seed = dir.join("seed");
    g(&seed, &["checkout", "-q", "-b", "feature"]);
    std::fs::write(seed.join("feature.txt"), "f\n").unwrap();
    g(&seed, &["add", "feature.txt"]);
    g(&seed, &["commit", "-q", "-m", "feature"]);
    g(&seed, &["push", "-q", "origin", "feature"]);
    m.fetch().unwrap();
    let other = dir.join("home/projects/p/worktrees/h-2");
    add(&m, &other, "feature", "main", &[]).unwrap();
    assert!(other.join("feature.txt").exists());
}

#[test]
fn a_sparse_worktree_holds_the_folders_asked_for_and_widens() {
    let (_t, dir) = tmp();
    let origin = origin(&dir, "main");
    let m = mirror(&dir, &origin, "main");
    m.fetch().unwrap();
    let tree = dir.join("home/projects/p/worktrees/h-3");
    let asked = folders(&["a/".to_string()]).unwrap();
    add(&m, &tree, "flotilla/h-3", "main", &asked).unwrap();
    assert!(tree.join("a/x/f").exists());
    assert!(tree.join("README").exists(), "files at the top always come");
    assert!(!tree.join("b").exists(), "b wasn't asked for");
    assert_eq!(
        g(&tree, &["status", "--porcelain"]),
        "",
        "nothing shows as deleted"
    );

    let now = widen(&tree, &asked, None).unwrap();
    assert_eq!(now, ["a"], "no ask leaves it as it is");
    let now = widen(&tree, &now, Some(&["b".to_string()])).unwrap();
    assert_eq!(now, ["a", "b"]);
    assert!(tree.join("b/g").exists());
    let now = widen(&tree, &now, Some(&[])).unwrap();
    assert!(now.is_empty(), "an empty ask makes it whole");
    // A whole tree already holds anything asked.
    assert!(widen(&tree, &now, Some(&["a".to_string()]))
        .unwrap()
        .is_empty());
}

#[test]
fn a_failed_worktree_leaves_nothing_behind() {
    let (_t, dir) = tmp();
    let origin = origin(&dir, "main");
    let m = mirror(&dir, &origin, "main");
    m.fetch().unwrap();
    let tree = dir.join("home/projects/p/worktrees/h-4");
    assert!(add(&m, &tree, "flotilla/h-4", "no-such-base", &[]).is_err());
    assert!(!tree.exists());
    assert_eq!(g(&m.path, &["for-each-ref", "refs/heads"]), "");
}

#[test]
fn folders_are_relative_plain_and_deduplicated() {
    let ok = folders(&[
        "src/app/".to_string(),
        "./docs".to_string(),
        "src\\app".to_string(),
    ])
    .unwrap();
    assert_eq!(ok, ["docs", "src/app"]);
    for bad in [
        "", "/etc", "../up", "a/../b", "a//b", "-x", ":(glob)*", "a*", "C:/x",
    ] {
        assert!(folders(&[bad.to_string()]).is_err(), "{bad:?}");
    }
    let many: Vec<String> = (0..=MAX_FOLDERS).map(|i| format!("d{i}")).collect();
    assert!(folders(&many).is_err());
}

#[test]
fn a_cards_branch_is_its_linked_one_or_named_from_it() {
    let (_t, dir) = tmp();
    let origin = origin(&dir, "main");
    let m = mirror(&dir, &origin, "main");
    m.fetch().unwrap();
    let card = |branches: &[&str]| Card {
        id: "H-012".into(),
        title: "Search: by title & tag!".into(),
        branches: branches.iter().map(|b| b.to_string()).collect(),
    };
    assert_eq!(
        branch_of(&m.path, &card(&[])),
        "flotilla/h-012-search-by-title-tag"
    );
    assert_eq!(branch_of(&m.path, &card(&["H-12-search"])), "H-12-search");
    assert_eq!(
        branch_of(&m.path, &card(&["-bad", "a..b"])),
        "flotilla/h-012-search-by-title-tag",
        "a link git wouldn't take as a branch is passed over"
    );
    assert!(looks_like_card("PHD-7") && !looks_like_card("x") && !looks_like_card("A-b"));
}

/// The mirror's config is the bots' to write (they commit in its trees):
/// a hook or fsmonitor planted there never runs in the daemon's git.
#[cfg(unix)]
#[test]
fn hooks_planted_in_the_mirror_never_run() {
    let (_t, dir) = tmp();
    let origin = origin(&dir, "main");
    let m = mirror(&dir, &origin, "main");
    m.fetch().unwrap();
    let marker = dir.join("marker");
    let hooks = dir.join("hooks");
    std::fs::create_dir_all(&hooks).unwrap();
    for hook in ["post-checkout", "reference-transaction", "fsmonitor"] {
        let script = hooks.join(hook);
        std::fs::write(
            &script,
            format!("#!/bin/sh\necho ran >> '{}'\n", marker.display()),
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    g(
        &m.path,
        &["config", "core.hooksPath", &hooks.display().to_string()],
    );
    let fsmonitor = hooks.join("fsmonitor").display().to_string();
    g(&m.path, &["config", "core.fsmonitor", &fsmonitor]);
    let tree = dir.join("home/projects/p/worktrees/h-5");
    add(&m, &tree, "flotilla/h-5", "main", &["a".to_string()]).unwrap();
    widen(&tree, &["a".to_string()], Some(&["b".to_string()])).unwrap();
    m.fetch().unwrap();
    assert!(!marker.exists(), "a planted program ran");
    // The planting works: plain git in the tree runs it.
    g(&tree, &["checkout", "-q", "-b", "planted"]);
    assert!(marker.exists());
}
