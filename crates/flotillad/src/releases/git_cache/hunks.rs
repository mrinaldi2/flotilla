//! Line hunks between two commits in the cache (moved out of git_cache.rs
//! for the 400-line limit).

use std::path::Path;

use super::{git, is_blob, ok};

/// One hunk of a zero-context diff: old lines `old_start..old_start+old_len`
/// became `new_len` lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hunk {
    pub old_start: u32,
    pub old_len: u32,
    pub new_len: u32,
}

/// The hunks that turn `path` at `from` into `path` at `to`, or `None` when
/// `to` has no such file. No textconv or external diff runs (SafeGit).
pub fn hunks(cache: &Path, from: &str, to: &str, path: &str) -> anyhow::Result<Option<Vec<Hunk>>> {
    if !is_blob(cache, to, path)? {
        return Ok(None);
    }
    let out = git(
        cache,
        &[
            "diff",
            "-U0",
            "--no-color",
            "--no-ext-diff",
            "--no-textconv",
            from,
            to,
            "--",
            path,
        ],
    )?;
    ok(&out, "diff")?;
    let text = String::from_utf8_lossy(&out.stdout);
    let hunks: Vec<Hunk> = text.lines().filter_map(parse_hunk).collect();
    // Changed with no line hunks (binary, or `-diff`): no line maps across.
    if hunks.is_empty() && !text.trim().is_empty() {
        return Ok(None);
    }
    Ok(Some(hunks))
}

/// How many lines `path` has at `commit`: `None` when it isn't a text file
/// there.
pub fn line_count(cache: &Path, commit: &str, path: &str) -> anyhow::Result<Option<usize>> {
    if !is_blob(cache, commit, path)? {
        return Ok(None);
    }
    let out = git(cache, &["cat-file", "blob", &format!("{commit}:{path}")])?;
    ok(&out, "cat-file")?;
    Ok(String::from_utf8(out.stdout)
        .ok()
        .map(|t| t.lines().count()))
}

/// `@@ -a[,b] +c[,d] @@`, where a missing count is 1.
fn parse_hunk(line: &str) -> Option<Hunk> {
    let rest = line.strip_prefix("@@ -")?;
    let (old, rest) = rest.split_once(" +")?;
    let new = rest.split_once(" @@")?.0;
    let range = |s: &str| -> Option<(u32, u32)> {
        match s.split_once(',') {
            Some((start, len)) => Some((start.parse().ok()?, len.parse().ok()?)),
            None => Some((s.parse().ok()?, 1)),
        }
    };
    let (old_start, old_len) = range(old)?;
    let (_, new_len) = range(new)?;
    Some(Hunk {
        old_start,
        old_len,
        new_len,
    })
}
