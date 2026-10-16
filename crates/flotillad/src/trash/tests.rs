use std::path::Path;

use super::confine::{check, plain_name, Depth};
use super::*;

fn cfg_in(home: &Path) -> Config {
    Config {
        home: home.join("home"),
        user_home: home.to_path_buf(),
        ..Config::default()
    }
}

#[test]
fn an_item_must_be_plainly_inside_its_root() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("root");
    std::fs::create_dir_all(root.join("a/b")).unwrap();
    std::fs::create_dir_all(dir.path().join("outside")).unwrap();
    assert!(check(&root.join("a"), &root, Depth::Child).is_ok());
    assert!(check(&root.join("a/b"), &root, Depth::Below).is_ok());
    assert!(
        check(&root.join("a/b"), &root, Depth::Child).is_err(),
        "too deep"
    );
    assert!(
        check(&root, &root, Depth::Below).is_err(),
        "the root itself"
    );
    assert!(check(&root.join("a/../../outside"), &root, Depth::Below).is_err());
    assert!(check(&dir.path().join("outside"), &root, Depth::Below).is_err());
    assert!(check(&root.join("missing"), &root, Depth::Child).is_err());
    assert!(
        check(Path::new("root/a"), &root, Depth::Child).is_err(),
        "relative"
    );
    #[cfg(unix)]
    {
        // A link planted as the item, or as a folder on the way to it,
        // never leads the trash outside the root.
        std::os::unix::fs::symlink(dir.path().join("outside"), root.join("link")).unwrap();
        assert!(check(&root.join("link"), &root, Depth::Child).is_err());
        std::fs::create_dir_all(dir.path().join("outside/inner")).unwrap();
        assert!(check(&root.join("link/inner"), &root, Depth::Below).is_err());
    }
}

#[test]
fn a_name_is_one_plain_part() {
    assert!(plain_name("acme"));
    assert!(plain_name("p1.git"));
    for bad in ["", ".", "..", "a/b", "/abs", "../x"] {
        assert!(!plain_name(bad), "{bad}");
    }
}

#[test]
fn test_builds_use_the_trash_folder_never_the_os_trash() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = cfg_in(dir.path());
    assert_eq!(first_bin(&cfg), Bin::Folder);
    let item = cfg.home.join("projects/acme");
    std::fs::create_dir_all(&item).unwrap();
    std::fs::write(item.join("notes.md"), "kept").unwrap();
    let bucket = Bucket {
        label: "acme",
        group: "project-files",
    };
    assert_eq!(move_to_trash(&cfg, &item, bucket), Ok(Bin::Folder));
    assert!(!item.exists());
    let moved = bucket_dir(&cfg, bucket).join("acme");
    assert_eq!(
        std::fs::read_to_string(moved.join("notes.md")).unwrap(),
        "kept"
    );

    // A second item of the same name is numbered, not overwritten.
    std::fs::create_dir_all(&item).unwrap();
    assert_eq!(move_to_trash(&cfg, &item, bucket), Ok(Bin::Folder));
    assert!(bucket_dir(&cfg, bucket).join("acme-2").is_dir());
}

#[test]
fn the_sweep_empties_only_old_dated_buckets() {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = cfg_in(dir.path());
    cfg.trash.fallback_days = 30;
    let trash = fallback_root(&cfg);
    for name in ["2001-01-01-old", "notes", "2001-01-01"] {
        std::fs::create_dir_all(trash.join(name)).unwrap();
    }
    let today = bucket_dir(
        &cfg,
        Bucket {
            label: "new",
            group: "g",
        },
    );
    std::fs::create_dir_all(&today).unwrap();
    assert_eq!(sweep(&cfg), 2);
    assert!(!trash.join("2001-01-01-old").exists());
    assert!(trash.join("notes").exists(), "not a dated bucket");
    assert!(today.exists(), "not old yet");
}

#[test]
fn sizes_count_files_and_never_follow_links() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("item");
    std::fs::create_dir_all(root.join("sub")).unwrap();
    std::fs::write(root.join("a"), vec![0u8; 100]).unwrap();
    std::fs::write(root.join("sub/b"), vec![0u8; 50]).unwrap();
    assert_eq!(size_of(&root), 150);
    #[cfg(unix)]
    {
        let big = dir.path().join("big");
        std::fs::write(&big, vec![0u8; 10_000]).unwrap();
        std::os::unix::fs::symlink(&big, root.join("link")).unwrap();
        assert!(size_of(&root) < 1_000, "the link counts as itself");
    }
}
