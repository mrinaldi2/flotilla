use super::*;
use crate::safe_git::canonical;
use crate::safe_git::tests::plain;

#[test]
fn only_relative_repository_paths_reach_git() {
    for good in [
        "docs/a.md",
        "docs/initiatives/x/prd.md",
        "README.md",
        "img/a b.png",
    ] {
        assert_eq!(check_path(good).expect(good), good);
    }
    for bad in [
        "",
        "/etc/passwd",
        "../secret",
        "docs/../../x",
        "docs/./a.md",
        "docs//a.md",
        ":(glob)docs/*",
        "docs\\a.md",
        "docs/a\0.md",
        "docs/",
    ] {
        assert!(check_path(bad).is_err(), "{bad:?}");
    }
    assert!(check_doc_path("docs/x/prd.md").is_ok());
    let outside = check_doc_path("src/main.rs").unwrap_err().to_string();
    assert!(outside.contains("docs/"), "{outside}");
}

#[test]
fn a_commit_is_hex_and_nothing_else() {
    assert!(check_sha("9b2f0c1").is_ok());
    assert!(check_sha(&"a".repeat(40)).is_ok());
    for bad in [
        "HEAD",
        "main",
        "abc",
        "--output=x",
        "9b2f0c1^",
        &"a".repeat(65),
    ] {
        assert!(check_sha(bad).is_err(), "{bad}");
    }
}

#[test]
fn a_document_is_served_as_its_extension_says() {
    assert_eq!(mime_of("docs/x/prd.md"), "text/markdown");
    assert_eq!(mime_of("docs/x/index.HTML"), "text/html");
    assert_eq!(mime_of("docs/x/shot.png"), "image/png");
    assert_eq!(mime_of("docs/x/a.svg"), "image/svg+xml");
    assert_eq!(mime_of("docs/x/spec.pdf"), "application/pdf");
    assert_eq!(mime_of("docs/x/blob"), "application/octet-stream");
}

/// A repository whose main has a guideline and a PRD, then a second commit
/// changing the PRD; the daemon's blob-less cache of it.
fn repo() -> (tempfile::TempDir, std::path::PathBuf, String, String) {
    let dir = tempfile::tempdir().unwrap();
    let root = canonical(dir.path()).unwrap();
    let work = root.join("work");
    std::fs::create_dir_all(work.join("docs/guidelines/code")).unwrap();
    std::fs::create_dir_all(work.join("docs/x")).unwrap();
    plain(&work, &["init", "-q", "-b", "main"]);
    plain(&work, &["config", "user.email", "t@t"]);
    plain(&work, &["config", "user.name", "t"]);
    std::fs::write(work.join("docs/guidelines/style.md"), "# Style\n").unwrap();
    std::fs::write(work.join("docs/guidelines/code/rust.md"), "# Rust\n").unwrap();
    std::fs::write(work.join("docs/x/prd.md"), "# PRD\none\n").unwrap();
    std::fs::write(work.join("docs/x/shot.png"), b"\x89PNG\r\n\x1a\n\xff\x00").unwrap();
    plain(&work, &["add", "."]);
    plain(&work, &["commit", "-q", "-m", "one"]);
    let first = plain(&work, &["rev-parse", "HEAD"]);
    std::fs::write(work.join("docs/x/prd.md"), "# PRD\ntwo\n").unwrap();
    plain(&work, &["commit", "-q", "-am", "two"]);
    let second = plain(&work, &["rev-parse", "HEAD"]);
    plain(&root, &["clone", "-q", "--bare", "work", "cache.git"]);
    (dir, root.join("cache.git"), first, second)
}

fn doc(path: &str, approved: Option<&str>) -> Doc {
    Doc {
        kind: DocKind::Prd,
        path: path.into(),
        approved_sha: approved.map(str::to_string),
        approved_at: None,
        changed_since_approval: false,
        item_id: None,
    }
}

#[test]
fn documents_are_read_listed_and_compared_from_the_cache() {
    let (_dir, cache, first, second) = repo();
    let mut listed = git_cache::files_under(&cache, &second, GUIDELINES).unwrap();
    listed.sort();
    assert_eq!(
        listed,
        ["docs/guidelines/code/rust.md", "docs/guidelines/style.md"]
    );
    let png = git_cache::bytes_at(&cache, &second, "docs/x/shot.png")
        .unwrap()
        .expect("there")
        .expect("a file");
    assert_eq!(png, b"\x89PNG\r\n\x1a\n\xff\x00");
    assert!(git_cache::bytes_at(&cache, &second, "docs/x/none.md")
        .unwrap()
        .is_none());
    assert!(git_cache::bytes_at(&cache, &second, "docs/x")
        .unwrap()
        .expect("an entry")
        .is_err());
    // The PRD changed after the commit it was approved at; the guideline
    // didn't.
    assert!(changed_since_approval(
        &cache,
        &second,
        &doc("docs/x/prd.md", Some(&first))
    ));
    assert!(!changed_since_approval(
        &cache,
        &second,
        &doc("docs/x/prd.md", Some(&second))
    ));
    let style = doc("docs/guidelines/style.md", Some(&first));
    assert!(!changed_since_approval(&cache, &second, &style));
    assert!(!changed_since_approval(
        &cache,
        &second,
        &doc("docs/x/prd.md", None)
    ));
    let diff = git_cache::diff(&cache, &first, &second, Some("docs/x/prd.md"), &[]).unwrap();
    let diff = String::from_utf8(diff).unwrap();
    assert!(diff.contains("-one") && diff.contains("+two"), "{diff}");
}

/// The plan's reads run nothing the cache's config names (H-289): a planted
/// external diff, textconv and filter driver never run.
#[test]
fn document_reads_run_no_planted_program() {
    let (dir, cache, first, second) = repo();
    let root = canonical(dir.path()).unwrap();
    let marker = root.join("marker");
    let driver = root.join("driver");
    std::fs::write(
        &driver,
        format!("#!/bin/sh\necho ran >> '{}'\ncat\n", marker.display()),
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&driver, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let driver = driver.display().to_string();
    #[cfg(windows)]
    let driver = driver.replace('\\', "/");
    for (key, value) in [
        ("diff.external", driver.as_str()),
        ("diff.planted.textconv", driver.as_str()),
        ("filter.planted.smudge", driver.as_str()),
        ("filter.planted.clean", driver.as_str()),
    ] {
        plain(&cache, &["config", key, value]);
    }
    std::fs::create_dir_all(cache.join("info")).unwrap();
    std::fs::write(
        cache.join("info").join("attributes"),
        "*.md diff=planted filter=planted\n",
    )
    .unwrap();
    // The planting works: plain git runs the driver.
    plain(&cache, &["diff", &first, &second]);
    assert!(marker.exists(), "the planted driver runs under plain git");
    std::fs::remove_file(&marker).unwrap();
    let path = "docs/x/prd.md";
    let text = git_cache::bytes_at(&cache, &second, path)
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(text, b"# PRD\ntwo\n");
    git_cache::files_under(&cache, &second, GUIDELINES).unwrap();
    git_cache::diff(&cache, &first, &second, Some(path), &[]).unwrap();
    assert!(changed_since_approval(
        &cache,
        &second,
        &doc(path, Some(&first))
    ));
    assert!(!marker.exists(), "a plan read ran a planted command");
}
