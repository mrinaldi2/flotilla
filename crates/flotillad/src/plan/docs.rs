//! The plan's documents in the project's repository, read through the
//! daemon's own cache of it (`releases::git_cache`), never a bot's
//! checkout. Only a path of the repository, relative and without `..`, and
//! only a commit id in hex reach git (H-289).

use std::path::{Path, PathBuf};

use bus::contract::board as c;

use super::model::{Doc, DocKind};
use crate::app::AppState;
use crate::decisions::{invalid, not_found};
use crate::prs::diff::capped;
use crate::prs::repo;
use crate::releases::git_cache;

/// The most bytes one `plan_doc_read` answer carries.
pub const CHUNK_MAX: usize = 512 * 1024;

/// Where the project's guidelines live, read by every bot before it works.
pub const GUIDELINES: &str = "docs/guidelines";

/// A path of the repository, as git may be given it: relative, with no
/// `..`, no `\`, no NUL and no pathspec magic.
pub fn check_path(path: &str) -> anyhow::Result<&str> {
    let path = path.trim();
    let bad = path.is_empty()
        || path.starts_with('/')
        || path.starts_with(':')
        || path.contains('\0')
        || path.contains('\\')
        || path
            .split('/')
            .any(|part| part == ".." || part == "." || part.is_empty());
    if bad {
        return Err(invalid(format!(
            "{path:?} isn't a path in the repository: give it relative to the repository's \
             root, like docs/initiatives/x/prd.md"
        )));
    }
    Ok(path)
}

/// A plan document's path: a repository path under `docs/`.
pub fn check_doc_path(path: &str) -> anyhow::Result<&str> {
    let path = check_path(path)?;
    if !path.starts_with("docs/") {
        return Err(invalid(format!(
            "{path} isn't under docs/: the plan's documents live there"
        )));
    }
    Ok(path)
}

/// A commit id as given, in hex; nothing else reaches git.
pub fn check_sha(sha: &str) -> anyhow::Result<&str> {
    let sha = sha.trim();
    let hex = (4..=64).contains(&sha.len()) && sha.bytes().all(|b| b.is_ascii_hexdigit());
    if !hex {
        return Err(invalid(format!("{sha:?} isn't a commit id")));
    }
    Ok(sha)
}

/// The type a document's file is served as, from its extension.
pub fn mime_of(path: &str) -> &'static str {
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    match ext.as_str() {
        "md" | "markdown" => "text/markdown",
        "html" | "htm" => "text/html",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "pdf" => "application/pdf",
        "txt" => "text/plain",
        "json" => "application/json",
        _ => "application/octet-stream",
    }
}

/// The project's repository in the daemon's cache, and its default
/// branch's head. `fetch` refreshes the cache first; `None` when neither the
/// remote nor the cache can tell.
pub fn head(app: &AppState, project_id: &str, fetch: bool) -> Option<(PathBuf, String)> {
    let branch = app.db.project_repo(project_id).ok()??.branch;
    let repo = repo::of_project(app, project_id, None).ok()?;
    let cached = repo.cached(app);
    let cache = match fetch || !cached.join("HEAD").exists() {
        true => repo.fetch(app).unwrap_or_else(|e| {
            tracing::debug!(project_id, error = %e, "plan: repository fetch failed; using the cache");
            cached
        }),
        false => cached,
    };
    if !cache.join("HEAD").exists() {
        return None;
    }
    let branch = if branch.trim().is_empty() {
        "main"
    } else {
        branch.trim()
    };
    // A project set to a branch its repository doesn't have (the default
    // `main` where the repository uses `develop`) reads the default branch.
    let sha = git_cache::resolve(&cache, &format!("refs/heads/{branch}"))
        .or_else(|| git_cache::resolve(&cache, "HEAD"))?;
    Some((cache, sha))
}

/// The guidelines on the default branch, when the repository can be read.
pub fn guidelines(app: &AppState, project_id: &str) -> Vec<Doc> {
    let Some((cache, sha)) = head(app, project_id, false) else {
        return Vec::new();
    };
    git_cache::files_under(&cache, &sha, GUIDELINES)
        .unwrap_or_default()
        .into_iter()
        .map(|path| Doc {
            kind: DocKind::Guideline,
            path,
            approved_sha: None,
            approved_at: None,
            changed_since_approval: false,
            item_id: None,
        })
        .collect()
}

/// Whether `doc` differs on the default branch (`head`) from the commit the
/// owner approved it at. A document never approved, or a commit the cache
/// doesn't hold, reads as unchanged.
pub fn changed_since_approval(cache: &Path, head: &str, doc: &Doc) -> bool {
    let Some(approved) = doc.approved_sha.as_deref() else {
        return false;
    };
    if check_sha(approved).is_err() || check_path(&doc.path).is_err() {
        return false;
    }
    let blob = |commit: &str| match git_cache::blob_at(cache, commit, &doc.path) {
        Ok(Some(Ok(object))) => Some(object),
        _ => None,
    };
    if !git_cache::has(cache, approved) {
        return false;
    }
    blob(approved) != blob(head)
}

/// One chunk of a file of the project's repository, at `sha` or the default
/// branch's head, with the diff from `diff_from` on the first chunk.
pub fn read(app: &AppState, r: &c::PlanDocRead) -> anyhow::Result<c::PlanDocContent> {
    let path = check_path(&r.path)?;
    if app.db.project_repo(&r.project_id)?.is_none() {
        return Err(not_found("this project has no repository"));
    }
    // The first chunk reads the remote's latest; the rest, the same commit.
    let (cache, head) = head(app, &r.project_id, r.offset == 0)
        .ok_or_else(|| not_found("the project's repository can't be read now"))?;
    let sha = match r.sha.as_deref().filter(|s| !s.trim().is_empty()) {
        Some(sha) => commit(&cache, check_sha(sha)?)?,
        None => head,
    };
    let bytes = match git_cache::bytes_at(&cache, &sha, path)? {
        Some(Ok(bytes)) => bytes,
        Some(Err(why)) => return Err(invalid(why)),
        None => {
            return Err(not_found(format!(
                "{path} isn't in the repository at {sha}"
            )))
        }
    };
    let total = bytes.len() as u64;
    let limit = match r.limit {
        0 => CHUNK_MAX,
        n => (n as usize).min(CHUNK_MAX),
    };
    let start = r.offset.min(total) as usize;
    let end = (start + limit).min(bytes.len());
    let diff = match r.diff_from.as_deref().filter(|d| !d.trim().is_empty()) {
        Some(from) if r.offset == 0 => {
            let from = commit(&cache, check_sha(from)?)?;
            let raw = git_cache::diff(&cache, &from, &sha, Some(path), &[])?;
            Some(capped(&raw).0)
        }
        _ => None,
    };
    Ok(c::PlanDocContent {
        path: path.to_string(),
        sha,
        offset: start as u64,
        data: bytes[start..end].to_vec(),
        total,
        mime: mime_of(path).to_string(),
        diff,
    })
}

/// The full id of a commit the cache holds.
fn commit(cache: &Path, sha: &str) -> anyhow::Result<String> {
    git_cache::resolve(cache, sha)
        .filter(|full| git_cache::has(cache, full))
        .ok_or_else(|| not_found(format!("{sha} isn't a commit of this repository")))
}

#[cfg(test)]
#[path = "docs_tests.rs"]
mod tests;
