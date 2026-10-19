//! A card's worktree (0.23.0, `worktree_for`): made from the project's
//! mirror on the card's branch, one per card and shared by every bot on it,
//! in `<project>/worktrees/<card>`. It is sparse to the folders asked for,
//! when some are; a later ask can widen it. Once the card's PR merges the
//! cleanup after merge removes it like any worktree of the branch; a
//! cancelled card's is salvaged, then removed ([`super::tidy`]).

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use super::mirror::{git, Mirror};
use crate::app::AppState;
use crate::db::CardWorktree;
use crate::kanban::model::{ColumnCategory, LinkKind};

/// At most this many folders in one ask.
pub const MAX_FOLDERS: usize = 64;
const MAX_FOLDER_LEN: usize = 300;

/// The folders a sparse worktree is asked for, checked and in one spelling:
/// relative, `/`-separated, no `.` or `..` part and nothing git would read
/// as a pattern.
pub fn folders(asked: &[String]) -> anyhow::Result<Vec<String>> {
    anyhow::ensure!(
        asked.len() <= MAX_FOLDERS,
        "at most {MAX_FOLDERS} folders; ask for their parent instead"
    );
    let mut out = Vec::new();
    for raw in asked {
        let path = raw.trim().replace('\\', "/");
        let path = path.trim_start_matches("./").trim_end_matches('/');
        let bad = path.is_empty()
            || path.len() > MAX_FOLDER_LEN
            || path.starts_with('/')
            || path.starts_with('-')
            || path.contains(':')
            || path
                .chars()
                .any(|c| c.is_control() || "*?[]!#\"'`$".contains(c))
            || path.split('/').any(|part| matches!(part, "" | "." | ".."));
        anyhow::ensure!(
            !bad,
            "{raw:?} isn't a folder of the repository; give paths like src/app"
        );
        out.push(path.to_string());
    }
    out.sort();
    out.dedup();
    Ok(out)
}

/// What `worktree_for` needs of the card.
pub struct Card {
    pub id: String,
    pub title: String,
    /// The branch linked on the card, newest first.
    pub branches: Vec<String>,
}

fn looks_like_card(id: &str) -> bool {
    id.split_once('-').is_some_and(|(key, seq)| {
        !key.is_empty()
            && key.chars().all(|c| c.is_ascii_alphanumeric())
            && !seq.is_empty()
            && seq.chars().all(|c| c.is_ascii_digit())
    })
}

/// The card `id` of `bot`'s project. A board kept on a linked computer has
/// no copy here, so there any card id is taken as given.
pub fn card(app: &AppState, bot: &bus::Bot, id: &str) -> anyhow::Result<Card> {
    let id = id.trim();
    let ours = app.db.item_project(id)?.as_deref() == Some(bot.project_id.as_str());
    match app.db.get_item(id)?.filter(|_| ours) {
        Some(item) => {
            anyhow::ensure!(
                !matches!(
                    item.category,
                    ColumnCategory::Done | ColumnCategory::Cancelled
                ),
                "{id} is {}: it gets no worktree",
                item.category.as_str()
            );
            let mut links = app.db.item_links(id)?;
            links.retain(|l| l.kind == LinkKind::Branch);
            links.sort_by_key(|l| std::cmp::Reverse(l.at));
            Ok(Card {
                id: item.id,
                title: item.title,
                branches: links.into_iter().map(|l| l.target).collect(),
            })
        }
        None if looks_like_card(id) && !app.db.project_links(&bot.project_id)?.is_empty() => {
            Ok(Card {
                id: id.to_string(),
                title: String::new(),
                branches: Vec::new(),
            })
        }
        None => anyhow::bail!("no card {id} in this project"),
    }
}

/// Lowercase letters, digits and `-`, at most `max` long.
fn slug(text: &str, max: usize) -> String {
    let mut out = String::new();
    for c in text.chars().map(|c| c.to_ascii_lowercase()) {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let mut out: String = out.trim_matches('-').chars().take(max).collect();
    while out.ends_with('-') {
        out.pop();
    }
    out
}

/// The card's branch: the one linked on it, else `flotilla/<card>-<title>`.
pub fn branch_of(mirror: &Path, card: &Card) -> String {
    let valid =
        |b: &str| !b.starts_with('-') && git(mirror, &["check-ref-format", "--branch", b]).is_ok();
    if let Some(linked) = card.branches.iter().find(|b| valid(b)) {
        return linked.clone();
    }
    let name = match slug(&card.title, 40) {
        t if t.is_empty() => slug(&card.id, 40),
        t => format!("{}-{t}", slug(&card.id, 40)),
    };
    format!("{}/{name}", crate::brand::ON_DISK_SLUG)
}

/// The card's folder in the project's worktrees.
pub fn folder(app: &AppState, mirror: &Mirror, card_id: &str) -> PathBuf {
    crate::paths::card_worktrees_dir(&app.cfg, &mirror.project_dir).join(slug(card_id, 60))
}

/// Whether `tree` is still a linked worktree (a `.git` file, not a link).
pub fn is_worktree(tree: &Path) -> bool {
    std::fs::symlink_metadata(tree.join(".git")).is_ok_and(|m| m.is_file())
}

/// Adds the worktree at `dest` on `branch`: the local branch if the mirror
/// has one, else the remote's, else a new one from `base`. Sparse to
/// `folders` when there are some. Nothing is left behind if it fails.
pub fn add(
    mirror: &Mirror,
    dest: &Path,
    branch: &str,
    base: &str,
    folders: &[String],
) -> anyhow::Result<()> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    git(&mirror.path, &["worktree", "prune"])?;
    let shown = crate::safe_git::plain_path(dest).display().to_string();
    let mut args = vec!["worktree", "add", "--quiet"];
    if !folders.is_empty() {
        args.push("--no-checkout");
    }
    let remote = format!("refs/remotes/origin/{branch}");
    let start = match mirror.resolve(&remote) {
        Some(_) => remote,
        None => format!("refs/remotes/origin/{base}"),
    };
    if mirror.resolve(&format!("refs/heads/{branch}")).is_some() {
        args.extend([shown.as_str(), branch]);
    } else {
        args.extend(["--no-track", "-b", branch, shown.as_str(), start.as_str()]);
    }
    git(&mirror.path, &args)?;
    let finish = || -> anyhow::Result<()> {
        // `git push` and `git pull` in the tree go to the card's branch.
        git(
            &mirror.path,
            &["config", &format!("branch.{branch}.remote"), "origin"],
        )?;
        let merge = format!("refs/heads/{branch}");
        git(
            &mirror.path,
            &["config", &format!("branch.{branch}.merge"), &merge],
        )?;
        if !folders.is_empty() {
            let mut set = vec!["sparse-checkout", "set", "--cone", "--"];
            set.extend(folders.iter().map(String::as_str));
            git(dest, &set)?;
            git(dest, &["read-tree", "-mu", "HEAD"])?;
        }
        Ok(())
    };
    if let Err(error) = finish() {
        let _ = git(&mirror.path, &["worktree", "remove", "--force", &shown]);
        return Err(error);
    }
    Ok(())
}

/// The folders `tree` holds once `asked` is added: `None` keeps it as it
/// is, an empty ask makes it whole, and a whole tree already holds all.
pub fn widen(tree: &Path, now: &[String], asked: Option<&[String]>) -> anyhow::Result<Vec<String>> {
    match asked {
        None => Ok(now.to_vec()),
        Some(_) if now.is_empty() => Ok(Vec::new()),
        Some([]) => {
            git(tree, &["sparse-checkout", "disable"])?;
            Ok(Vec::new())
        }
        Some(asked) => {
            let new: Vec<&str> = asked
                .iter()
                .filter(|a| !now.contains(a))
                .map(String::as_str)
                .collect();
            if new.is_empty() {
                return Ok(now.to_vec());
            }
            let mut add = vec!["sparse-checkout", "add", "--"];
            add.extend(new);
            git(tree, &add)?;
            let mut all: Vec<String> = now.iter().chain(asked).cloned().collect();
            all.sort();
            all.dedup();
            Ok(all)
        }
    }
}

fn answer(tree: &CardWorktree, created: bool) -> Value {
    json!({
        "item_id": tree.item_id, "path": tree.path, "branch": tree.branch,
        "base": tree.base, "paths": tree.paths, "created": created,
    })
}

/// `worktree_for {item_id, paths?}` for `bot`: the card's worktree, made
/// now or as it already is (widened to `paths`).
pub fn worktree_for(
    app: &AppState,
    bot: &bus::Bot,
    item_id: &str,
    paths: Option<&[String]>,
) -> anyhow::Result<Value> {
    let asked = paths.map(folders).transpose()?;
    let card = card(app, bot, item_id)?;
    let mirror = Mirror::of_project(app, &bot.project_id)?;
    let lock = mirror.lock();
    let _held = lock.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(mut tree) = app.db.card_worktree(&mirror.project_id, &card.id)? {
        if is_worktree(Path::new(&tree.path)) {
            let now = widen(Path::new(&tree.path), &tree.paths, asked.as_deref())?;
            if now != tree.paths {
                app.db
                    .set_card_worktree_paths(&tree.project_id, &tree.item_id, &now)?;
                tree.paths = now;
            }
            return Ok(answer(&tree, false));
        }
        // Gone from disk (removed by hand, or cleaned up): made again.
        app.db
            .remove_card_worktree(&tree.project_id, &tree.item_id)?;
    }
    if let Err(error) = mirror.fetch_locked() {
        anyhow::ensure!(mirror.exists(), "couldn't fetch the repository: {error:#}");
        tracing::warn!(%error, "mirror fetch failed; the worktree starts from what it has");
    }
    let base = mirror.base()?;
    let branch = branch_of(&mirror.path, &card);
    let dest = folder(app, &mirror, &card.id);
    anyhow::ensure!(
        std::fs::symlink_metadata(&dest).is_err(),
        "{} is in the way; move it aside and ask again",
        dest.display()
    );
    let folders = asked.unwrap_or_default();
    add(&mirror, &dest, &branch, &base, &folders)?;
    super::seed::warm(&dest);
    let tree = CardWorktree {
        project_id: mirror.project_id.clone(),
        item_id: card.id,
        path: crate::safe_git::canonical(&dest)?.display().to_string(),
        branch,
        base,
        paths: folders,
        bot_id: bot.id.clone(),
        created_at: chrono::Utc::now().to_rfc3339(),
    };
    app.db.add_card_worktree(&tree)?;
    Ok(answer(&tree, true))
}

#[cfg(test)]
#[path = "card_tests.rs"]
pub(crate) mod tests;
