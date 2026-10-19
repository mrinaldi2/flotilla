//! Claude Code's own worktrees, `<repo>/.claude/worktrees/<name>` (0.22.0):
//! a bot's when the name starts with one of its names (its folder name, and
//! `lead` for the project's lead), unless another bot's name is a longer
//! match (`unity-dev-2-…` is Unity Dev 2's, not Unity Dev's).

use std::path::Path;

/// Whether `rest` (a path under a trusted path) is in one of the bot's
/// Claude Code worktrees.
pub(super) fn owns(own: &[String], others: &[String], rest: &Path) -> bool {
    let parts: Vec<String> = rest
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_ascii_lowercase())
        .collect();
    let Some(at) = parts
        .windows(2)
        .position(|w| w[0] == ".claude" && w[1] == "worktrees")
    else {
        return false;
    };
    // The repository folder comes first: a `.claude` right in a trusted
    // path is nobody's.
    let Some(name) = parts.get(at + 2).filter(|_| at >= 1) else {
        return false;
    };
    let longest = |names: &[String]| {
        names
            .iter()
            .filter(|n| {
                !n.is_empty()
                    && name
                        .strip_prefix(n.as_str())
                        .is_some_and(|tail| tail.is_empty() || tail.starts_with('-'))
            })
            .map(String::len)
            .max()
    };
    match (longest(own), longest(others)) {
        (Some(mine), Some(theirs)) => mine >= theirs,
        (Some(_), None) => true,
        _ => false,
    }
}
