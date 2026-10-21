//! Push requests (0.24.0, `push_requests_v1`): a bot that needs a push it
//! may not make asks with `request_push`, and the owner taps Push. It is an
//! owner action of `kind: "push"` whose content is exactly
//! `git push origin <branch>:<target>`, run in the bot's worktree, with the
//! details the owner reads (`PushDetails`) taken from that worktree when
//! the bot asked. A push to the project's branch or to `main` is a release,
//! not a button, so it is refused.

use std::path::Path;
use std::sync::Arc;

use crate::app::AppState;
use crate::decisions::invalid;
use crate::safe_git::SafeGit;

use super::model::{OwnerAction, Proposal, PushDetails, Shell};
use super::{store, validate, writable_roots};

/// What a bot asks to have pushed.
#[derive(Default)]
pub struct PushRequest<'a> {
    pub branch: &'a str,
    /// The remote branch; `branch` when left out.
    pub target: Option<&'a str>,
    pub item_id: Option<&'a str>,
    pub reason: &'a str,
    /// The worktree it runs in; by default the bot's card worktree on
    /// `branch`.
    pub worktree: Option<&'a str>,
}

fn git(dir: &Path, args: &[&str]) -> Option<String> {
    SafeGit::local(dir)
        .ok()?
        .args(args)
        .run()
        .ok()
        .map(|out| out.trim().to_string())
}

/// A branch name git would take and a shell reads as one word.
fn check_branch(what: &str, name: &str) -> anyhow::Result<()> {
    let plain = name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '/'));
    let well_formed = !name.is_empty()
        && !name.starts_with(['-', '/', '.'])
        && !name.ends_with(['/', '.'])
        && !name.contains("..")
        && !name.contains("//")
        && !name.ends_with(".lock");
    if plain && well_formed {
        return Ok(());
    }
    Err(invalid(format!(
        "{what} {name:?} isn't a branch name: letters, digits, '.', '_', '-' and '/' only"
    )))
}

/// The branch a push may never land on: the project's own and `main`.
fn protected(app: &AppState, project_id: &str) -> anyhow::Result<Vec<String>> {
    let mut out = vec!["main".to_string()];
    if let Some(repo) = app.db.project_repo(project_id)? {
        let branch = repo.branch.trim();
        if !branch.is_empty() && branch != "main" {
            out.push(branch.to_string());
        }
    }
    Ok(out)
}

/// The worktree the push runs in: the one named, or the bot's card worktree
/// on `branch`. Checked as a PR's worktree is: the bot may work there, it's
/// a git worktree, and it is on `branch`.
fn worktree(app: &AppState, bot: &bus::Bot, req: &PushRequest<'_>) -> anyhow::Result<String> {
    let machine = app.db.daemon_id()?;
    let inspect = |path: &str| {
        crate::prs::worktree::inspect(app, bot, &machine, Some(req.branch), path)
            .map(|(tree, _, _)| tree.path)
    };
    if let Some(path) = req.worktree.map(str::trim).filter(|p| !p.is_empty()) {
        return inspect(path).map_err(|e| invalid(format!("{e:#}")));
    }
    let trees = app.db.card_worktrees(Some(&bot.project_id))?;
    let mut candidates: Vec<_> = trees.iter().filter(|t| t.branch == req.branch).collect();
    // The card's own first, then the bot's.
    candidates.sort_by_key(|t| (Some(t.item_id.as_str()) != req.item_id, t.bot_id != bot.id));
    candidates
        .into_iter()
        .find_map(|t| inspect(&t.path).ok())
        .ok_or_else(|| {
            invalid(format!(
                "no worktree of yours is on {}: name it (worktree), checked out on that branch",
                req.branch
            ))
        })
}

/// Files changed and lines added and removed by `commit` over what the
/// remote has: its `target` when it exists there, else the project's branch.
fn diff_stat(dir: &Path, commit: &str, bases: &[&str]) -> (u32, u32, u32) {
    let base = bases
        .iter()
        .map(|b| format!("refs/remotes/origin/{b}"))
        .find(|r| git(dir, &["rev-parse", "--verify", "--quiet", r]).is_some());
    let Some(base) = base else {
        return (0, 0, 0);
    };
    let range = format!("{base}...{commit}");
    let numstat = git(dir, &["diff", "--numstat", &range]).unwrap_or_default();
    numstat.lines().fold((0, 0, 0), |(files, add, del), line| {
        let mut cols = line.split('\t');
        let n = |c: Option<&str>| c.and_then(|v| v.parse::<u32>().ok()).unwrap_or(0);
        let (a, d) = (n(cols.next()), n(cols.next()));
        (files + 1, add + a, del + d)
    })
}

/// The bot's push request, stored as a waiting owner action.
pub fn request(
    app: &Arc<AppState>,
    bot: &bus::Bot,
    req: &PushRequest<'_>,
) -> anyhow::Result<OwnerAction> {
    let branch = req.branch.trim();
    let target = req
        .target
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .unwrap_or(branch);
    check_branch("branch", branch)?;
    check_branch("target", target)?;
    let protected = protected(app, &bot.project_id)?;
    if protected.iter().any(|p| p == target) {
        return Err(invalid(format!(
            "{target} is the project's branch: a push there is a release, not a push request. \
             Push to a branch of your own and open a PR or link it on the card."
        )));
    }
    validate::visible("the reason", req.reason)?;
    if req.reason.trim().is_empty() {
        return Err(invalid("say why the owner should push it (reason)"));
    }
    if let Some(item) = req.item_id {
        if app.db.item_project(item)?.as_deref() != Some(bot.project_id.as_str()) {
            return Err(invalid(format!("no item {item} in this project")));
        }
    }
    let req = PushRequest { branch, ..*req };
    let cwd = worktree(app, bot, &req)?;
    let dir = Path::new(&cwd);
    let commit = git(
        dir,
        &["rev-parse", "--verify", &format!("refs/heads/{branch}")],
    )
    .ok_or_else(|| invalid(format!("{branch} has no commit in {cwd}")))?;
    let origin = git(dir, &["config", "--get", "remote.origin.url"])
        .ok_or_else(|| invalid(format!("{cwd} has no origin to push to")))?;
    let mut bases = vec![target];
    bases.extend(protected.iter().rev().map(String::as_str));
    let (files, additions, deletions) = diff_stat(dir, &commit, &bases);
    let push = PushDetails {
        branch: branch.to_string(),
        target: target.to_string(),
        repo: crate::prs::repo::name_of(&origin),
        commit,
        files,
        additions,
        deletions,
        item_id: req.item_id.map(str::to_string),
    };
    let proposal = Proposal {
        project_id: bot.project_id.clone(),
        proposed_by: format!("bot:{}", bot.id),
        item_id: req.item_id.map(str::to_string),
        decision_id: None,
        target_machine: app.db.daemon_id()?,
        shell: Shell::here(),
        cwd,
        content: format!("git push origin {branch}:{target}"),
        pinned_files: Vec::new(),
        reason: req.reason.trim().to_string(),
        timeout_s: validate::timeout(None)?,
        push: Some(push),
    };
    store(app, proposal, "bot", writable_roots(app))
}

/// Why a push request can't run now: its branch moved since the bot asked,
/// as a changed pinned file stops a command.
pub fn drift(a: &OwnerAction) -> Option<String> {
    let push = a.proposal.push.as_ref()?;
    let spec = format!("refs/heads/{}", push.branch);
    let now = git(
        Path::new(&a.proposal.cwd),
        &["rev-parse", "--verify", &spec],
    )
    .unwrap_or_else(|| "missing".to_string());
    (now != push.commit).then(|| {
        format!(
            "Refused: {} moved since the push was requested: {} → {now}\n",
            push.branch, push.commit
        )
    })
}

#[cfg(test)]
mod tests {
    use super::check_branch;

    #[test]
    fn only_plain_branch_names_are_taken() {
        for ok in ["feature/x", "flotilla/ch-1", "v1.2_fix"] {
            assert!(check_branch("branch", ok).is_ok(), "{ok}");
        }
        for bad in [
            "", "-f", "a b", "a..b", "x;rm", "+main", "a/", "x.lock", "$(id)",
        ] {
            assert!(check_branch("branch", bad).is_err(), "{bad}");
        }
    }
}
