//! Warm seeds (0.23.0): a new card worktree starts with the dependencies the
//! newest of its siblings already installed, copied with APFS `clonefile`
//! (`cp -c`), so unchanged files cost no extra disk and the install that
//! follows has little left to do. Only where the disk clones: on any other
//! system, or a disk that can't, nothing is copied.

use std::path::{Path, PathBuf};

/// The folders a new worktree is seeded with.
pub const SEEDED: &[&str] = &["node_modules"];

/// Seeds `tree` from its newest sibling in the same folder. The names
/// seeded.
pub fn warm(tree: &Path) -> Vec<String> {
    let Some(siblings) = tree.parent() else {
        return Vec::new();
    };
    let mut seeded = Vec::new();
    for name in SEEDED {
        let Some(from) = newest(siblings, tree, name) else {
            continue;
        };
        let to = tree.join(name);
        if std::fs::symlink_metadata(&to).is_err() && clone_dir(&from, &to) {
            seeded.push((*name).to_string());
        }
    }
    seeded
}

/// A real folder (not a link).
fn plain_dir(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|m| m.is_dir() && !crate::cleanup::scope::is_link(&m))
}

/// `<sibling>/<name>` of the sibling of `tree` that changed it last.
fn newest(siblings: &Path, tree: &Path, name: &str) -> Option<PathBuf> {
    std::fs::read_dir(siblings)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p != tree && plain_dir(p))
        .map(|p| p.join(name))
        .filter(|p| plain_dir(p))
        .filter_map(|p| {
            let at = std::fs::metadata(&p).and_then(|m| m.modified()).ok()?;
            Some((at, p))
        })
        .max_by_key(|(at, _)| *at)
        .map(|(_, p)| p)
}

/// Clones the folder `from` to `to` with `clonefile`, links kept as links.
/// False, with nothing left at `to`, when the disk can't.
#[cfg(target_os = "macos")]
pub fn clone_dir(from: &Path, to: &Path) -> bool {
    let cloned = std::process::Command::new("/bin/cp")
        .arg("-c")
        .arg("-R")
        .arg(from)
        .arg(to)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success());
    if !cloned {
        let _ = std::fs::remove_dir_all(to);
    }
    cloned
}

/// Elsewhere a copy would cost the full size again: none is made.
#[cfg(not(target_os = "macos"))]
pub fn clone_dir(_from: &Path, _to: &Path) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_tree_is_seeded_from_its_newest_sibling_only_where_the_disk_clones() {
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("H-1");
        let new = dir.path().join("H-2");
        let tree = dir.path().join("H-3");
        for (sibling, text) in [(&old, "old"), (&new, "new")] {
            std::fs::create_dir_all(sibling.join("node_modules/pkg")).unwrap();
            std::fs::write(sibling.join("node_modules/pkg/index.js"), text).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        std::fs::create_dir_all(&tree).unwrap();
        let seeded = warm(&tree);
        let copied = tree.join("node_modules/pkg/index.js");
        if cfg!(target_os = "macos") {
            // macOS temp folders are on APFS.
            assert_eq!(seeded, ["node_modules"]);
            assert_eq!(std::fs::read_to_string(&copied).unwrap(), "new");
            // A second seed never overwrites what the tree has.
            assert!(warm(&tree).is_empty());
        } else {
            assert!(seeded.is_empty());
            assert!(
                !copied.exists(),
                "nothing is copied where the disk can't clone"
            );
        }
    }

    #[test]
    fn a_tree_with_no_sibling_is_left_as_it_is() {
        let dir = tempfile::tempdir().unwrap();
        let tree = dir.path().join("H-1");
        std::fs::create_dir_all(&tree).unwrap();
        assert!(warm(&tree).is_empty());
        assert!(!tree.join("node_modules").exists());
    }
}
