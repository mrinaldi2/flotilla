//! The read-only repo tools (0.23.0): `repo_file`, `repo_diff`, `repo_log`
//! and `repo_grep` read the project's mirror at any branch, tag or commit,
//! so a bot that only reads needs no worktree. Every answer is capped, and
//! every git here is plumbing through the hardened runner with nothing that
//! could run a program (no textconv, external diff or signature check).

use std::path::Path;

use serde_json::{json, Value};

use super::mirror::Mirror;
use crate::safe_git::SafeGit;

/// A file's text is cut here…
pub const MAX_FILE: usize = 512 * 1024;
/// …and a file bigger than this isn't read at all.
pub const MAX_READ: u64 = 32 * 1024 * 1024;
/// A folder lists at most this many entries.
pub const MAX_ENTRIES: usize = 1000;
/// A diff's patch is cut here, on a whole line.
pub const MAX_DIFF: usize = 256 * 1024;
pub const DEFAULT_LOG: u32 = 30;
pub const MAX_LOG: u32 = 200;
/// `repo_grep` gives at most this many matching lines…
pub const MAX_MATCHES: usize = 300;
/// …each cut to this many characters.
pub const MAX_LINE: usize = 400;
pub const MAX_PATTERN: usize = 300;

fn output(mirror: &Path, args: &[&str]) -> anyhow::Result<std::process::Output> {
    SafeGit::local(mirror)?.args(args).output()
}

fn ok(out: std::process::Output, what: &str) -> anyhow::Result<Vec<u8>> {
    anyhow::ensure!(
        out.status.success(),
        "git {what}: {}",
        String::from_utf8_lossy(&out.stderr).trim()
    );
    Ok(out.stdout)
}

/// A path in the repository, checked: relative, `/`-separated, no `.` or
/// `..` part, nothing git reads as pathspec magic. Empty is the top.
pub fn path_arg(raw: &str) -> anyhow::Result<String> {
    let path = raw.trim().replace('\\', "/");
    let path = path.trim_start_matches("./").trim_end_matches('/');
    if path.is_empty() || path == "." {
        return Ok(String::new());
    }
    let bad = path.len() > 1024
        || path.starts_with('/')
        || path.starts_with(':')
        || path.starts_with('-')
        || path.chars().any(char::is_control)
        || path.split('/').any(|part| matches!(part, "" | "." | ".."));
    anyhow::ensure!(!bad, "{raw:?} isn't a path in the repository");
    Ok(path.to_string())
}

/// A ref as given (a branch, a tag, a commit, with `~n`/`^`), resolved in
/// the mirror: the remote's branch first, then a tag, then as written. No
/// ref means the branch new work starts from. Its name and commit.
pub fn ref_arg(mirror: &Mirror, raw: Option<&str>) -> anyhow::Result<(String, String)> {
    let name = match raw.map(str::trim).filter(|r| !r.is_empty()) {
        Some(r) => r.to_string(),
        None => mirror.base()?,
    };
    let allowed = |c: char| c.is_ascii_alphanumeric() || "._/~^-".contains(c);
    let bad = name.len() > 200
        || name.starts_with(['-', '/'])
        || name.contains("..")
        || !name.chars().all(allowed);
    anyhow::ensure!(!bad, "{name:?} isn't a branch, tag or commit");
    let candidates = match name.strip_prefix("origin/") {
        Some(_) => vec![format!("refs/remotes/{name}")],
        None => vec![
            format!("refs/remotes/origin/{name}"),
            format!("refs/tags/{name}"),
            name.clone(),
        ],
    };
    let commit = candidates
        .iter()
        .find_map(|c| mirror.resolve(c))
        .ok_or_else(|| anyhow::anyhow!("no branch, tag or commit {name} in the repository"))?;
    Ok((name, commit))
}

/// Text cut to `max` bytes on a whole line (or a character, for one long
/// line), and whether it was cut.
pub fn cut(text: &str, max: usize) -> (&str, bool) {
    if text.len() <= max {
        return (text, false);
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let end = text[..end].rfind('\n').map_or(end, |n| n + 1);
    (&text[..end], true)
}

/// `repo_file {path, ref?}`: a file's text, or a folder's entries.
pub fn file(mirror: &Mirror, path: &str, reference: Option<&str>) -> anyhow::Result<Value> {
    let path = path_arg(path)?;
    let (name, commit) = ref_arg(mirror, reference)?;
    let tree = |listing: Vec<u8>| {
        let text = String::from_utf8_lossy(&listing).into_owned();
        let entries: Vec<Value> = text
            .split('\0')
            .filter_map(|e| e.split_once('\t'))
            .take(MAX_ENTRIES)
            .map(|(meta, n)| {
                let kind = match meta.split(' ').nth(1) {
                    Some("tree") => "folder",
                    Some("commit") => "submodule",
                    _ if meta.starts_with("120000") => "link",
                    _ => "file",
                };
                json!({"name": n, "type": kind})
            })
            .collect();
        let count = text.split('\0').filter(|e| e.contains('\t')).count();
        json!({"ref": name, "commit": commit, "path": path, "type": "folder",
               "entries": entries, "truncated": count > MAX_ENTRIES})
    };
    if path.is_empty() {
        return Ok(tree(ok(
            output(&mirror.path, &["ls-tree", "-z", &commit])?,
            "ls-tree",
        )?));
    }
    let listed = ok(
        output(&mirror.path, &["ls-tree", "-z", &commit, "--", &path])?,
        "ls-tree",
    )?;
    let listed = String::from_utf8_lossy(&listed).into_owned();
    let Some((meta, _)) = listed
        .split('\0')
        .filter_map(|e| e.split_once('\t'))
        .find(|(_, n)| *n == path)
    else {
        anyhow::bail!("no {path} at {name}");
    };
    let mut meta = meta.split(' ');
    let (mode, kind, object) = (
        meta.next().unwrap_or_default(),
        meta.next().unwrap_or_default(),
        meta.next().unwrap_or_default(),
    );
    match kind {
        "tree" => {
            let inside = format!("{path}/");
            let listing = output(&mirror.path, &["ls-tree", "-z", &commit, "--", &inside])?;
            let mut out = tree(ok(listing, "ls-tree")?);
            // Entries are named from the top; shown by their own name.
            if let Some(entries) = out["entries"].as_array_mut() {
                for e in entries {
                    let short = e["name"].as_str().unwrap_or_default();
                    let short = short.strip_prefix(&inside).unwrap_or(short).to_string();
                    e["name"] = json!(short);
                }
            }
            Ok(out)
        }
        "commit" => Ok(json!({"ref": name, "commit": commit, "path": path,
                              "type": "submodule", "at": object})),
        _ => {
            let size: u64 = String::from_utf8_lossy(&ok(
                output(&mirror.path, &["cat-file", "-s", object])?,
                "cat-file",
            )?)
            .trim()
            .parse()
            .unwrap_or(0);
            let mut out = json!({"ref": name, "commit": commit, "path": path, "size": size,
                                 "type": if mode == "120000" { "link" } else { "file" }});
            if size > MAX_READ {
                out["truncated"] = json!(true);
                out["note"] = json!("too big to read here; search it with repo_grep");
                return Ok(out);
            }
            let bytes = ok(
                output(&mirror.path, &["cat-file", "blob", object])?,
                "cat-file",
            )?;
            if bytes.iter().take(8000).any(|b| *b == 0) {
                out["binary"] = json!(true);
                return Ok(out);
            }
            let text = String::from_utf8_lossy(&bytes);
            let (shown, truncated) = cut(&text, MAX_FILE);
            out["text"] = json!(shown);
            out["truncated"] = json!(truncated);
            Ok(out)
        }
    }
}

/// `git diff --numstat -z`: each file's path (`old => new` when renamed)
/// and lines added and removed (none for a binary file).
fn numstat(raw: &[u8]) -> Vec<Value> {
    let text = String::from_utf8_lossy(raw);
    let mut parts = text.split('\0');
    let mut files = Vec::new();
    while let Some(head) = parts.next() {
        let mut fields = head.splitn(3, '\t');
        let (Some(added), Some(removed), Some(path)) =
            (fields.next(), fields.next(), fields.next())
        else {
            continue;
        };
        let path = if path.is_empty() {
            let old = parts.next().unwrap_or_default();
            let new = parts.next().unwrap_or_default();
            format!("{old} => {new}")
        } else {
            path.to_string()
        };
        files.push(json!({"path": path, "added": added.parse::<u64>().ok(),
                          "removed": removed.parse::<u64>().ok()}));
    }
    files
}

/// `repo_diff {base, head?, path?}`: what `head` changed since it left
/// `base` (`git diff base...head`).
pub fn diff(
    mirror: &Mirror,
    base: &str,
    head: Option<&str>,
    path: Option<&str>,
) -> anyhow::Result<Value> {
    let (base_name, base_commit) = ref_arg(mirror, Some(base))?;
    let (head_name, head_commit) = ref_arg(mirror, head)?;
    let path = path.map(path_arg).transpose()?.filter(|p| !p.is_empty());
    let from = super::mirror::git(&mirror.path, &["merge-base", &base_commit, &head_commit])
        .unwrap_or_else(|_| base_commit.clone());
    let run = |how: &[&str]| -> anyhow::Result<Vec<u8>> {
        let mut args = vec!["diff", "--no-color", "--no-ext-diff", "--no-textconv", "-M"];
        args.extend_from_slice(how);
        args.extend([from.as_str(), head_commit.as_str(), "--"]);
        args.extend(path.as_deref());
        ok(output(&mirror.path, &args)?, "diff")
    };
    let files = numstat(&run(&["--numstat", "-z"])?);
    let patch = run(&[])?;
    let patch = String::from_utf8_lossy(&patch);
    let (shown, truncated) = cut(&patch, MAX_DIFF);
    Ok(
        json!({"base": base_name, "head": head_name, "from": from, "to": head_commit,
              "files": files, "diff": shown, "truncated": truncated}),
    )
}

/// `repo_log {ref?, path?, limit?}`: the newest commits first.
pub fn log(
    mirror: &Mirror,
    reference: Option<&str>,
    path: Option<&str>,
    limit: Option<u32>,
) -> anyhow::Result<Value> {
    let (name, commit) = ref_arg(mirror, reference)?;
    let path = path.map(path_arg).transpose()?.filter(|p| !p.is_empty());
    let limit = limit.unwrap_or(DEFAULT_LOG).clamp(1, MAX_LOG).to_string();
    let mut args = vec![
        "log",
        "--no-color",
        "--no-show-signature",
        "--no-ext-diff",
        "--format=%H%x1f%an%x1f%aI%x1f%s%x1e",
        "-n",
        &limit,
        &commit,
        "--",
    ];
    args.extend(path.as_deref());
    let out = ok(output(&mirror.path, &args)?, "log")?;
    let text = String::from_utf8_lossy(&out);
    let commits: Vec<Value> = text
        .split('\x1e')
        .filter_map(|r| {
            let mut f = r.trim_start_matches('\n').split('\x1f');
            let sha = f.next().filter(|s| !s.is_empty())?;
            Some(json!({"sha": sha, "author": f.next(), "at": f.next(), "subject": f.next()}))
        })
        .collect();
    Ok(json!({"ref": name, "commit": commit, "path": path, "commits": commits}))
}

/// `repo_grep {pattern, ref?, path?}`: lines matching an extended regular
/// expression, in text files only.
pub fn grep(
    mirror: &Mirror,
    pattern: &str,
    reference: Option<&str>,
    path: Option<&str>,
) -> anyhow::Result<Value> {
    anyhow::ensure!(
        !pattern.is_empty() && pattern.len() <= MAX_PATTERN && !pattern.contains(['\n', '\0']),
        "give a pattern of one line, at most {MAX_PATTERN} characters"
    );
    let (name, commit) = ref_arg(mirror, reference)?;
    let path = path.map(path_arg).transpose()?.filter(|p| !p.is_empty());
    let mut args = vec![
        "grep",
        "--no-color",
        "--no-textconv",
        "-z",
        "-n",
        "-I",
        "-E",
        "-e",
        pattern,
        &commit,
        "--",
    ];
    args.extend(path.as_deref());
    let out = output(&mirror.path, &args)?;
    let raw = match out.status.code() {
        Some(1) => Vec::new(),
        _ => ok(out, "grep")?,
    };
    let text = String::from_utf8_lossy(&raw);
    let prefix = format!("{commit}:");
    let lines: Vec<&str> = text.lines().collect();
    let matches: Vec<Value> = lines
        .iter()
        .take(MAX_MATCHES)
        .filter_map(|l| {
            let mut f = l.splitn(3, '\0');
            let file = f.next()?;
            let line = f.next()?.parse::<u64>().ok()?;
            let shown = f.next().unwrap_or_default();
            let (shown, _) = cut(shown, MAX_LINE);
            Some(json!({"path": file.strip_prefix(&prefix).unwrap_or(file),
                        "line": line, "text": shown}))
        })
        .collect();
    Ok(
        json!({"ref": name, "commit": commit, "pattern": pattern, "path": path,
              "matches": matches, "truncated": lines.len() > MAX_MATCHES}),
    )
}

#[cfg(test)]
#[path = "read_tests.rs"]
mod tests;
