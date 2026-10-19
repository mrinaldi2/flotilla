use super::*;
use crate::checkouts::card::tests::{g, mirror, origin};

fn fetched() -> (tempfile::TempDir, std::path::PathBuf, Mirror) {
    let t = tempfile::tempdir().unwrap();
    let dir = crate::safe_git::canonical(t.path()).unwrap();
    let origin = origin(&dir, "main");
    let m = mirror(&dir, &origin, "main");
    m.fetch().unwrap();
    (t, dir, m)
}

#[test]
fn paths_and_refs_are_checked_before_git_sees_them() {
    assert_eq!(path_arg("./a/x/").unwrap(), "a/x");
    assert_eq!(path_arg("a\\x").unwrap(), "a/x");
    assert_eq!(path_arg(".").unwrap(), "");
    for bad in ["/etc/passwd", "../x", "a/../../x", ":(top)x", "-p", "a//b"] {
        assert!(path_arg(bad).is_err(), "{bad:?}");
    }
    let (_t, _dir, m) = fetched();
    for bad in ["--output=/tmp/x", "a..b", "main@{1}", "x y", "/abs"] {
        assert!(ref_arg(&m, Some(bad)).is_err(), "{bad:?}");
    }
    let (name, commit) = ref_arg(&m, None).unwrap();
    assert_eq!(name, "main");
    assert_eq!(ref_arg(&m, Some("origin/main")).unwrap().1, commit);
    assert_eq!(ref_arg(&m, Some(&commit[..10])).unwrap().1, commit);
    assert!(ref_arg(&m, Some("nope"))
        .unwrap_err()
        .to_string()
        .contains("no branch"));
}

#[test]
fn repo_file_reads_files_and_folders_at_a_ref() {
    let (_t, dir, m) = fetched();
    let readme = file(&m, "README", None).unwrap();
    assert_eq!(readme["text"], "readme\nsecond line\n");
    assert_eq!(readme["truncated"], false);
    let top = file(&m, "", None).unwrap();
    let names: Vec<&str> = top["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["README", "a", "b"]);
    let folder = file(&m, "a/x", None).unwrap();
    assert_eq!(folder["type"], "folder");
    assert_eq!(folder["entries"][0]["name"], "f");
    assert!(file(&m, "missing.txt", None).is_err());

    // A big file is cut on a line; a binary one gives no text.
    let seed = dir.join("seed");
    let line = "x".repeat(99) + "\n";
    std::fs::write(seed.join("big.txt"), line.repeat(MAX_FILE / 100 + 10)).unwrap();
    std::fs::write(seed.join("bin"), [0u8, 1, 2, 3]).unwrap();
    g(&seed, &["add", "."]);
    g(&seed, &["commit", "-q", "-m", "big"]);
    g(&seed, &["push", "-q", "origin", "main:topic"]);
    m.fetch().unwrap();
    assert!(file(&m, "big.txt", None).is_err(), "not on main yet");
    let big = file(&m, "big.txt", Some("topic")).unwrap();
    assert_eq!(big["truncated"], true);
    let text = big["text"].as_str().unwrap();
    assert!(text.len() <= MAX_FILE && text.ends_with('\n'));
    let bin = file(&m, "bin", Some("topic")).unwrap();
    assert_eq!(bin["binary"], true);
    assert!(bin.get("text").is_none());
}

#[test]
fn repo_diff_log_and_grep_read_the_mirror() {
    let (_t, dir, m) = fetched();
    let seed = dir.join("seed");
    std::fs::write(seed.join("b/g"), "beta\ngamma fn\n").unwrap();
    g(&seed, &["commit", "-q", "-am", "gamma"]);
    g(&seed, &["push", "-q", "origin", "main:topic"]);
    m.fetch().unwrap();

    let d = diff(&m, "main", Some("topic"), None).unwrap();
    assert_eq!(d["files"][0]["path"], "b/g");
    assert_eq!(d["files"][0]["added"], 1);
    assert!(d["diff"].as_str().unwrap().contains("+gamma fn"));
    let elsewhere = diff(&m, "main", Some("topic"), Some("a")).unwrap();
    assert!(elsewhere["files"].as_array().unwrap().is_empty());

    let l = log(&m, Some("topic"), None, Some(1)).unwrap();
    assert_eq!(l["commits"].as_array().unwrap().len(), 1);
    assert_eq!(l["commits"][0]["subject"], "gamma");
    let all = log(&m, Some("topic"), Some("README"), None).unwrap();
    assert_eq!(all["commits"][0]["subject"], "start");

    let found = grep(&m, "fn$", Some("topic"), None).unwrap();
    let hits: Vec<(String, u64)> = found["matches"]
        .as_array()
        .unwrap()
        .iter()
        .map(|h| {
            (
                h["path"].as_str().unwrap().to_string(),
                h["line"].as_u64().unwrap(),
            )
        })
        .collect();
    assert_eq!(hits, [("a/x/f".to_string(), 1), ("b/g".to_string(), 2)]);
    let under = grep(&m, "fn", Some("topic"), Some("b")).unwrap();
    assert_eq!(under["matches"].as_array().unwrap().len(), 1);
    assert!(
        grep(&m, "nothing-like-this", None, None).unwrap()["matches"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(grep(&m, "", None, None).is_err());
    assert!(grep(&m, &"x".repeat(MAX_PATTERN + 1), None, None).is_err());
}

#[test]
fn many_matches_are_capped() {
    let (_t, dir, m) = fetched();
    let seed = dir.join("seed");
    std::fs::write(seed.join("many"), "hit\n".repeat(MAX_MATCHES + 50)).unwrap();
    g(&seed, &["add", "many"]);
    g(&seed, &["commit", "-q", "-m", "many"]);
    g(&seed, &["push", "-q", "origin", "main"]);
    m.fetch().unwrap();
    let found = grep(&m, "^hit$", None, None).unwrap();
    assert_eq!(found["matches"].as_array().unwrap().len(), MAX_MATCHES);
    assert_eq!(found["truncated"], true);
}

#[test]
fn text_is_cut_on_a_whole_line_or_character() {
    assert_eq!(cut("ab\ncd\n", 4), ("ab\n", true));
    assert_eq!(cut("ab\ncd\n", 10), ("ab\ncd\n", false));
    assert_eq!(cut("ééé", 3), ("é", true));
}
