//! What the daemon may trash: an item inside a known root, spelled from that
//! root down with no `..`, no link or junction in any part below the root
//! (the item itself included), and still inside the root once resolved. A
//! crafted bot path or a planted link therefore can't make the daemon trash
//! anything outside the folders it owns. Checked when the item is found and
//! again right before it moves.

use std::path::{Component, Path};

/// How far below its root an item may be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Depth {
    /// Directly in the root: `<root>/<name>`.
    Child,
    /// Anywhere below it.
    Below,
}

/// Why `path` may not be trashed as an item of `root`, if it may not.
pub fn check(path: &Path, root: &Path, depth: Depth) -> Result<(), String> {
    let shown = path.display();
    let plain = path.is_absolute()
        && path
            .components()
            .all(|c| !matches!(c, Component::ParentDir | Component::CurDir));
    if !plain {
        return Err(format!("{shown} is not a plain absolute path"));
    }
    let Ok(rest) = path.strip_prefix(root) else {
        return Err(format!("{shown} is outside {}", root.display()));
    };
    let parts: Vec<_> = rest.components().collect();
    if parts.is_empty() {
        return Err(format!("{shown} is the root itself"));
    }
    if depth == Depth::Child && parts.len() != 1 {
        return Err(format!("{shown} is not directly in {}", root.display()));
    }
    let mut at = root.to_path_buf();
    for part in parts {
        at.push(part);
        match std::fs::symlink_metadata(&at) {
            Ok(meta) if crate::cleanup::scope::is_link(&meta) => {
                return Err(format!(
                    "{shown} is reached through a link at {}",
                    at.display()
                ));
            }
            Ok(_) => {}
            Err(e) => return Err(format!("{}: {e}", at.display())),
        }
    }
    let real = crate::safe_git::canonical(path).map_err(|e| format!("{shown}: {e}"))?;
    let real_root =
        crate::safe_git::canonical(root).map_err(|e| format!("{}: {e}", root.display()))?;
    if real == real_root || !real.starts_with(&real_root) {
        return Err(format!("{shown} resolves outside {}", root.display()));
    }
    Ok(())
}

/// A single plain name: no separator, no `.` or `..`, nothing empty.
pub fn plain_name(name: &str) -> bool {
    let mut parts = Path::new(name).components();
    matches!(parts.next(), Some(Component::Normal(_))) && parts.next().is_none()
}
