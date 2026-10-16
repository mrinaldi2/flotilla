//! Moving a project's leftovers to the trash: each item checked again right
//! before it moves, anything a running process still holds skipped and
//! reported, a worktree pruned from its main clone once it is gone.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::{json, Value};

use super::{groups_text, size_text, Group, Item, Leftovers, Refused, Scope};
use crate::app::AppState;
use crate::cleanup::scope::Roots;
use crate::trash::{self, Bin, Bucket};

/// One item moved.
#[derive(Debug, Clone, Serialize)]
pub struct Moved {
    pub group: Group,
    pub path: String,
    pub bytes: u64,
    pub bin: Bin,
}

/// What a run moved and what it left.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Report {
    pub moved: Vec<Moved>,
    /// Found, but left where it is: held, refused, or the move failed.
    pub skipped: Vec<Refused>,
}

impl Report {
    pub fn moved_bytes(&self) -> u64 {
        self.moved.iter().map(|m| m.bytes).sum()
    }

    pub fn absorb(&mut self, other: Report) {
        self.moved.extend(other.moved);
        self.skipped.extend(other.skipped);
    }

    /// The bytes moved in each group, in the owner's order.
    pub fn by_group(&self) -> Vec<(Group, u64)> {
        Group::ALL
            .iter()
            .filter_map(|group| {
                let moved: Vec<&Moved> = self.moved.iter().filter(|m| m.group == *group).collect();
                (!moved.is_empty()).then(|| (*group, moved.iter().map(|m| m.bytes).sum()))
            })
            .collect()
    }

    /// Where the moved items went, as the owner calls it.
    pub fn bin_text(&self) -> &'static str {
        if self.moved.iter().any(|m| m.bin == Bin::Folder) {
            trash::bin_name(Bin::Folder)
        } else {
            trash::bin_name(Bin::Os)
        }
    }

    /// "Moved 1.6 GB to the Trash: project files 380 MB, …. 1 item was left: …"
    pub fn sentence(&self) -> String {
        let mut text = if self.moved.is_empty() {
            "Nothing was moved to the trash.".to_string()
        } else {
            format!(
                "Moved {} to the {}: {}.",
                size_text(self.moved_bytes()),
                self.bin_text(),
                groups_text(&self.by_group())
            )
        };
        match self.skipped.as_slice() {
            [] => {}
            [one] => text.push_str(&format!(" Left {}: {}.", one.path, one.reason)),
            many => text.push_str(&format!(
                " {} items were left where they are; the first: {}.",
                many.len(),
                many[0].reason
            )),
        }
        text
    }

    pub fn view(&self) -> Value {
        json!({
            "moved": self.moved,
            "moved_bytes": self.moved_bytes(),
            "skipped": self.skipped,
            "bin": self.bin_text(),
        })
    }
}

/// Moves every item of `leftovers` to the trash. Refused items are
/// reported, never touched.
pub fn trash(app: &AppState, leftovers: &Leftovers) -> Report {
    let mut report = Report {
        skipped: leftovers.refused.clone(),
        ..Report::default()
    };
    let mut items: Vec<&Item> = leftovers.items.iter().collect();
    // The project's folder last: a worktree or a history folder is checked
    // against the bot folders inside it.
    items.sort_by_key(|i| (i.group == Group::ProjectFiles, i.group));
    let held = wait_until_free(app, &items);
    for item in items {
        let shown = item.path.display().to_string();
        if let Some(why) = held.get(&item.path) {
            tracing::warn!(path = %shown, why, "left in place: in use");
            report.skipped.push(skipped(&shown, why));
            continue;
        }
        if std::fs::symlink_metadata(&item.path).is_err() {
            continue;
        }
        if let Err(why) = recheck(app, item) {
            tracing::warn!(path = %shown, why, "left in place: refused");
            report.skipped.push(skipped(&shown, &why));
            continue;
        }
        // Windows: a working folder in it, which the Restart Manager
        // doesn't see, makes a rename probe fail. Elsewhere a no-op.
        if let Err(outcome) = crate::cleanup::remove::not_in_use(&item.path) {
            use crate::cleanup::model::Outcome;
            let why = match outcome {
                Outcome::Busy(why) | Outcome::Held(why) | Outcome::Failed(why) => why,
                Outcome::Done { .. } => "in use".to_string(),
            };
            report.skipped.push(skipped(&shown, &why));
            continue;
        }
        let bucket = Bucket {
            label: &leftovers.project.dir_name,
            group: item.group.dir(),
        };
        match trash::move_to_trash(&app.cfg, &item.path, bucket) {
            Ok(bin) => {
                if let Scope::Worktree { main_clone, .. } = &item.scope {
                    prune(main_clone);
                }
                report.moved.push(Moved {
                    group: item.group,
                    path: shown,
                    bytes: item.bytes,
                    bin,
                });
            }
            Err(why) => {
                tracing::warn!(path = %shown, why, "left in place: the move failed");
                report.skipped.push(skipped(&shown, &why));
            }
        }
    }
    tracing::info!(
        project = %leftovers.project.name,
        moved = report.moved.len(),
        bytes = report.moved_bytes(),
        skipped = report.skipped.len(),
        "project leftovers trashed"
    );
    report
}

fn skipped(path: &str, reason: &str) -> Refused {
    Refused {
        path: path.to_string(),
        reason: reason.to_string(),
    }
}

/// The item's confinement, checked again on what is on disk now.
fn recheck(app: &AppState, item: &Item) -> Result<(), String> {
    match &item.scope {
        Scope::Under { root, depth } => trash::confine::check(&item.path, root, *depth),
        Scope::Worktree {
            workspace,
            bot_name,
            ..
        } => {
            Roots::of(app, workspace.as_deref(), bot_name).check(&item.path)?;
            match std::fs::symlink_metadata(item.path.join(".git")) {
                Ok(meta) if meta.is_file() => Ok(()),
                _ => Err(format!(
                    "{} is no longer a linked worktree",
                    item.path.display()
                )),
            }
        }
    }
}

/// Drops the moved worktree from its main clone's list.
fn prune(main_clone: &Path) {
    let pruned = crate::safe_git::SafeGit::local(main_clone)
        .and_then(|git| git.args(&["worktree", "prune"]).run());
    if let Err(e) = pruned {
        tracing::warn!(main = %main_clone.display(), error = %e, "worktree prune failed");
    }
}

/// Waits, up to `trash.held_wait_seconds`, for the sessions just stopped to
/// let go of the items, and returns what still holds each item that is held.
fn wait_until_free(app: &AppState, items: &[&Item]) -> BTreeMap<PathBuf, String> {
    let deadline = Instant::now() + Duration::from_secs(app.cfg.trash.held_wait_seconds);
    loop {
        let held = holders_of(items);
        if held.is_empty() || Instant::now() >= deadline {
            return held;
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}

/// What holds each held item: a process with its working folder or an open
/// file in it. When that can't be told, every item counts as held.
fn holders_of(items: &[&Item]) -> BTreeMap<PathBuf, String> {
    let spelled: Vec<(PathBuf, Vec<PathBuf>)> = items
        .iter()
        .filter(|i| i.path.exists())
        .map(|i| (i.path.clone(), crate::holders::spellings(&i.path)))
        .collect();
    let roots: Vec<PathBuf> = spelled.iter().flat_map(|(_, s)| s.clone()).collect();
    if roots.is_empty() {
        return BTreeMap::new();
    }
    let holders = match crate::holders::list(&roots) {
        Ok(holders) => holders,
        Err(error) => {
            let why = format!("couldn't tell whether it is in use: {error:#}");
            return spelled.into_iter().map(|(p, _)| (p, why.clone())).collect();
        }
    };
    let mut held = BTreeMap::new();
    for (path, spellings) in spelled {
        let these: Vec<_> = holders
            .iter()
            .filter(|h| spellings.iter().any(|s| h.path.starts_with(s)))
            .cloned()
            .collect();
        if !these.is_empty() {
            held.insert(path, format!("in use: {}", crate::holders::held_by(&these)));
        }
    }
    held
}
