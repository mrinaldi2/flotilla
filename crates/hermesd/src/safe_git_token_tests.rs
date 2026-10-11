//! H-321: the daemon's GitHub token reaches github.com only (S1) and is
//! never written into a repository's config (S2).

use std::path::Path;

use super::tests::plain;
use super::token::is_github;
use super::SafeGit;

const TOKEN: &str = "tok-h321-not-a-real-token";

/// A repository with one commit, made with plain git.
fn origin(root: &Path) -> std::path::PathBuf {
    let repo = super::canonical(root).unwrap().join("origin");
    std::fs::create_dir(&repo).unwrap();
    plain(&repo, &["init", "-q", "-b", "main"]);
    std::fs::write(repo.join("a.txt"), "one\n").unwrap();
    plain(&repo, &["add", "a.txt"]);
    plain(
        &repo,
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "commit",
            "-qm",
            "one",
        ],
    );
    repo
}

/// What `git credential fill` answers for `host` under the daemon's fetch.
fn fill(dir: &Path, host: &str) -> String {
    let asked = format!("protocol=https\nhost={host}\npath=o/r.git\n\n");
    let out = SafeGit::fetching_with(dir, Some(TOKEN.to_string()))
        .unwrap()
        .args(&["credential", "fill"])
        .output_with_stdin(asked.into_bytes())
        .unwrap();
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn only_github_com_over_https_counts_as_github() {
    for yes in [
        "https://github.com/mrinaldi2/gravity.git",
        "https://GitHub.com/o/r",
        "https://x-access-token@github.com/o/r",
    ] {
        assert!(is_github(yes), "{yes}");
    }
    for no in [
        "http://github.com/o/r",
        "https://github.com.example/o/r",
        "https://evil.example/github.com/o/r",
        "https://github.com@evil.example/o/r",
        "https://github.com:8443/o/r",
        "https://gist.github.com/o/r",
        "git@github.com:o/r.git",
        "/tmp/origin.git",
    ] {
        assert!(!is_github(no), "{no}");
    }
}

/// S1: the helper answers for github.com and for no other https host. git
/// asks for credentials by the host it really connects to, so this also
/// holds when the repository's own `url.*.insteadOf` sends github.com URLs
/// elsewhere: git then asks for that other host, and gets nothing.
#[test]
fn the_token_is_given_to_github_com_only() {
    let root = tempfile::tempdir().unwrap();
    let repo = origin(root.path());
    plain(
        &repo,
        &[
            "config",
            "url.https://evil.example/.insteadOf",
            "https://github.com/",
        ],
    );
    let github = fill(&repo, "github.com");
    assert!(github.contains(&format!("password={TOKEN}")), "{github}");
    for host in ["evil.example", "github.com.example", "gist.github.com"] {
        let other = fill(&repo, host);
        assert!(!other.contains(TOKEN), "{host}: {other}");
    }
    // No token at all for a fetch that isn't from github.com.
    for url in ["https://evil.example/o/r.git", &repo.display().to_string()] {
        assert!(SafeGit::fetching(&repo, url).unwrap().token_file.is_none());
    }
}

/// S2: a clone made with the token holds no credential and no token in its
/// config, and the token file is gone once the run is.
#[test]
fn a_clone_keeps_no_credential_or_token() {
    let root = tempfile::tempdir().unwrap();
    let origin = origin(root.path());
    let dest = root.path().join("clone");
    let git = SafeGit::fetching_with(root.path(), Some(TOKEN.to_string())).unwrap();
    let file = git.token_file.clone().expect("a token file");
    assert!(file.is_file());
    git.args(&[
        "clone",
        "--quiet",
        &origin.display().to_string(),
        &dest.display().to_string(),
    ])
    .run()
    .unwrap();
    assert!(!file.exists(), "the token file outlived the run");
    let config = std::fs::read_to_string(dest.join(".git").join("config")).unwrap();
    for word in ["credential", "x-access-token", TOKEN, "token-"] {
        assert!(
            !config.contains(word),
            "{word} in the clone's config:\n{config}"
        );
    }
}
