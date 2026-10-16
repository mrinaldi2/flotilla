//! Reading the policy from the base commit, never the head (H-267).

use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

use super::super::tests::{CHECKS, REVIEWERS};
use super::super::ReviewRole::{self, Architect, Ce, Owner};
use super::read;
use crate::releases::git_cache;

/// An empty global git config for the test repositories: Git for Windows
/// can't open `NUL` as a config file ("unable to access 'NUL'").
fn empty_config() -> &'static Path {
    static FILE: std::sync::OnceLock<tempfile::NamedTempFile> = std::sync::OnceLock::new();
    FILE.get_or_init(|| tempfile::NamedTempFile::new().unwrap())
        .path()
}

fn git(repo: &Path, args: &[&str]) {
    let out = Command::new("git")
        .current_dir(repo)
        .env("GIT_CONFIG_GLOBAL", empty_config())
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn write(repo: &Path, path: &str, text: &str) {
    let file = repo.join(path);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(file, text).unwrap();
}

fn commit(repo: &Path, message: &str) {
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-q", "-m", message]);
}

/// A repository whose main has the fixture policy files.
fn repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    git(repo, &["init", "-q", "-b", "main"]);
    write(repo, ".hermes/reviewers.toml", REVIEWERS);
    write(repo, ".hermes/checks.toml", CHECKS);
    write(repo, "README.md", "hi\n");
    commit(repo, "policy");
    dir
}

fn roles(list: &[ReviewRole]) -> BTreeSet<ReviewRole> {
    list.iter().copied().collect()
}

#[test]
fn a_pr_editing_the_policy_is_judged_by_the_base_rules() {
    let dir = repo();
    let repo = dir.path();
    git(repo, &["checkout", "-q", "-b", "sneaky"]);
    // The head drops every area and so would need only the architect.
    write(repo, ".hermes/reviewers.toml", "");
    write(repo, "crates/flotillad/src/peer/link.rs", "// change\n");
    commit(repo, "drop the reviewers");

    let base = read(repo, "main").unwrap();
    assert_eq!(base.reviewers.as_ref().unwrap().areas.len(), 7);
    assert_eq!(base.checks.as_ref().unwrap().checks.len(), 5);
    let changed = git_cache::changed_paths(repo, "main", "sneaky").unwrap();
    assert_eq!(
        changed,
        [
            ".hermes/reviewers.toml",
            "crates/flotillad/src/peer/link.rs"
        ]
    );
    assert_eq!(
        base.required_roles(&changed),
        roles(&[Architect, Ce, Owner])
    );

    // The head's own file has no say: read there, it has no areas at all.
    let head = read(repo, "sneaky").unwrap();
    assert!(head.reviewers.as_ref().unwrap().areas.is_empty());
}

#[test]
fn the_base_is_read_at_one_commit() {
    let dir = repo();
    let repo = dir.path();
    let base = read(repo, "main").unwrap();
    let head = git_cache::resolve(repo, "main").unwrap();
    assert_eq!(base.commit, head);
    assert!(read(repo, "no-such-branch").is_err());
}

#[test]
fn a_base_without_policy_files_needs_the_architect() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    git(repo, &["init", "-q", "-b", "main"]);
    write(repo, "README.md", "hi\n");
    commit(repo, "start");
    let base = read(repo, "main").unwrap();
    assert!(base.checks.as_ref().unwrap().checks.is_empty());
    assert_eq!(
        base.required_roles(&["src/x.rs".to_string()]),
        roles(&[Architect])
    );
}

/// A broken file on main can't lower the bar: every PR needs what a policy
/// change does until it is fixed, the fix included.
#[test]
fn a_broken_base_file_fails_closed() {
    let dir = repo();
    let repo = dir.path();
    write(repo, ".hermes/reviewers.toml", "[[area]]\nname = 'x'\n");
    write(repo, ".hermes/checks.toml", "[[check]]\nrun = 7\n");
    commit(repo, "break it");
    let base = read(repo, "main").unwrap();
    assert!(base
        .reviewers
        .as_ref()
        .unwrap_err()
        .contains(".hermes/reviewers.toml"));
    assert!(base
        .checks
        .as_ref()
        .unwrap_err()
        .contains(".hermes/checks.toml"));
    assert_eq!(
        base.required_roles(&["README.md".to_string()]),
        roles(&[Architect, Ce, Owner])
    );
}

/// `.flotilla/` is read before the pre-rename `.hermes/` when both exist.
#[test]
fn the_new_folder_wins_over_the_old_one() {
    let dir = repo();
    let repo = dir.path();
    write(repo, ".flotilla/reviewers.toml", "");
    commit(repo, "move the policy");
    let base = read(repo, "main").unwrap();
    assert!(base.reviewers.as_ref().unwrap().areas.is_empty());
    assert_eq!(
        base.checks.as_ref().unwrap().checks.len(),
        5,
        "still .hermes/"
    );
}

/// A bot adding the other folder's file beside an owner-reviewed one would
/// change the rules (the new folder wins): that needs the owner too, in
/// either direction.
#[test]
fn adding_the_other_folder_needs_the_owner() {
    for (base_dir, added) in [(".hermes", ".flotilla"), (".flotilla", ".hermes")] {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path();
        git(repo, &["init", "-q", "-b", "main"]);
        write(repo, &format!("{base_dir}/reviewers.toml"), REVIEWERS);
        commit(repo, "policy");
        git(repo, &["checkout", "-q", "-b", "sneaky"]);
        write(repo, &format!("{added}/reviewers.toml"), "");
        commit(repo, "a second policy");
        let base = read(repo, "main").unwrap();
        let changed = git_cache::changed_paths(repo, "main", "sneaky").unwrap();
        assert_eq!(changed, [format!("{added}/reviewers.toml")]);
        assert!(
            base.required_roles(&changed)
                .is_superset(&roles(&[Architect, Ce, Owner])),
            "{added} beside {base_dir}"
        );
        assert!(crate::prs::policy::touches_policy(&changed), "{added}");
    }
}

/// A directory (or anything not a file) at `.flotilla/reviewers.toml`
/// beside the owner's `.hermes/reviewers.toml` is a broken policy, which
/// needs the owner, not an error that stops every PR from opening or
/// merging.
#[test]
fn a_directory_at_a_policy_path_is_a_broken_policy() {
    let dir = repo();
    let repo = dir.path();
    write(repo, ".flotilla/reviewers.toml/README.md", "hi");
    write(repo, ".flotilla/checks.toml/README.md", "hi");
    commit(repo, "directories in the way");
    let base = read(repo, "main").unwrap();
    assert!(base
        .reviewers
        .as_ref()
        .unwrap_err()
        .contains(".flotilla/reviewers.toml"));
    assert!(base.checks.is_err());
    assert_eq!(
        base.required_roles(&["README.md".to_string()]),
        roles(&[Architect, Ce, Owner])
    );
}
