//! Keeping card worktrees tidy (0.23.0), every 10 minutes: each project's
//! mirror that exists is fetched; a cancelled card's worktree is salvaged,
//! then removed (`run::remove_anyway`: its uncommitted, untracked and
//! unpushed work is saved under `<home>/salvage/<project>/<card>` first, and
//! it stays if any of it can't be); a row whose worktree is gone is
//! forgotten. A merged card's goes with the cleanup after merge, which finds
//! it on the PR's branch (`cleanup::discover`).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use super::mirror::{git, Mirror};
use crate::app::AppState;
use crate::cleanup::model::Outcome;
use crate::cleanup::run::{self, Target};
use crate::db::CardWorktree;
use crate::kanban::model::ColumnCategory;

pub const EVERY: Duration = Duration::from_secs(600);

pub fn spawn(app: Arc<AppState>) {
    // A scratch daemon removes nothing (H-171).
    if app.cfg.scratch {
        return;
    }
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(EVERY);
        loop {
            tick.tick().await;
            let a = app.clone();
            if let Err(error) = tokio::task::spawn_blocking(move || pass(&a)).await {
                tracing::warn!(%error, "the card worktree pass failed");
            }
        }
    });
}

/// One pass: fetch, then each card worktree.
pub fn pass(app: &AppState) {
    for project in app.db.list_projects().unwrap_or_default() {
        let Ok(mirror) = Mirror::of_project(app, &project.id) else {
            continue;
        };
        if mirror.exists() {
            if let Err(error) = mirror.fetch() {
                tracing::warn!(project = %project.id, %error, "mirror fetch failed");
            }
        }
    }
    for tree in app.db.card_worktrees(None).unwrap_or_default() {
        if let Some(outcome) = tidy(app, &tree) {
            tracing::info!(item = %tree.item_id, path = %tree.path, ?outcome, "card worktree");
        }
    }
}

/// Whether the card is cancelled, or gone from a board kept here.
fn cancelled(app: &AppState, tree: &CardWorktree) -> bool {
    match app.db.get_item(&tree.item_id) {
        Ok(Some(item)) => item.category == ColumnCategory::Cancelled,
        Ok(None) => app
            .db
            .project_links(&tree.project_id)
            .is_ok_and(|links| links.is_empty()),
        Err(_) => false,
    }
}

/// Forgets a row whose tree is gone, and salvages then removes a cancelled
/// card's. What happened, if anything.
pub fn tidy(app: &AppState, tree: &CardWorktree) -> Option<Outcome> {
    let path = Path::new(&tree.path);
    let mirror = Mirror::of_project(app, &tree.project_id).ok();
    if std::fs::symlink_metadata(path).is_err() {
        if let Some(mirror) = mirror.as_ref().filter(|m| m.exists()) {
            let _ = git(&mirror.path, &["worktree", "prune"]);
        }
        let _ = app.db.remove_card_worktree(&tree.project_id, &tree.item_id);
        return Some(Outcome::Done { bytes: 0 });
    }
    if !cancelled(app, tree) {
        return None;
    }
    let outcome = remove_salvaged(app, tree, mirror.as_ref());
    if matches!(outcome, Outcome::Done { .. }) {
        let _ = app.db.remove_card_worktree(&tree.project_id, &tree.item_id);
    }
    Some(outcome)
}

/// Where a card's unsaved work goes before its worktree is removed.
pub fn salvage_dir(app: &AppState, tree: &CardWorktree) -> PathBuf {
    app.cfg
        .home
        .join("salvage")
        .join(&tree.project_id)
        .join(tree.item_id.replace(['/', '\\', ':'], "-"))
}

/// The card's tree removed once whatever it holds is saved: the owner's
/// Remove anyway, with the card's cancel as the word.
pub fn remove_salvaged(app: &AppState, tree: &CardWorktree, mirror: Option<&Mirror>) -> Outcome {
    let bot = app.db.get_bot(&tree.bot_id).ok().flatten();
    let target = Target {
        path: PathBuf::from(&tree.path),
        main_clone: mirror.map(|m| m.path.clone()),
        bot_name: bot.as_ref().map(|b| b.name.clone()).unwrap_or_default(),
        workspace: bot
            .map(|b| PathBuf::from(b.workspace_path))
            .filter(|w| w.is_absolute()),
        no_other_work: false,
    };
    let lock = mirror.map(Mirror::lock);
    let _held = lock
        .as_ref()
        .map(|l| l.lock().unwrap_or_else(|e| e.into_inner()));
    run::remove_anyway(app, &target, &salvage_dir(app, tree))
}
