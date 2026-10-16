//! How much an item would free: every file under it, links not followed.

use std::path::Path;

/// The bytes of every file under `path` (or of `path` itself), links
/// counted as themselves and never followed. Unreadable parts count 0.
pub fn size_of(path: &Path) -> u64 {
    let mut total = 0u64;
    let mut stack = vec![path.to_path_buf()];
    while let Some(next) = stack.pop() {
        let Ok(meta) = std::fs::symlink_metadata(&next) else {
            continue;
        };
        if meta.is_dir() && !crate::cleanup::scope::is_link(&meta) {
            if let Ok(entries) = std::fs::read_dir(&next) {
                stack.extend(entries.flatten().map(|e| e.path()));
            }
        } else {
            total = total.saturating_add(meta.len());
        }
    }
    total
}
