//! Deleting a project moves everything it left on disk to the trash
//! (0.19.0). Before, a delete archived the rows and kept every folder until
//! retention: Claude Code's session history alone left 1.5 GB behind for
//! one project, and nothing ever reclaimed it.
//!
//! What a project leaves, each found by [`collect`] and confined to its own
//! root ([`crate::trash::confine`]):
//!
//! - its folder, `<home>/projects/<dir>`, with every bot folder in it;
//! - each bot's Claude Code session history, `~/.claude/projects/<key>`
//!   ([`history`]), and the Codex session file of a Codex bot's thread;
//! - its git cache, `<home>/cache/repos/<project-id>.git`;
//! - the work saved from its worktrees, `<home>/salvage/<project-id>`;
//! - its bots' `<repo>-wt-<bot>` worktrees beside the owner's clones.
//!
//! Everything is moved to the trash ([`crate::trash`]), never erased, and an
//! item still held by a running process is skipped and reported. Linked
//! computers are told to do the same with their half ([`peer`]).

use std::path::PathBuf;

use bus::Project;
use serde::Serialize;
use serde_json::{json, Value};

pub mod cli;
pub mod collect;
pub mod history;
pub mod past;
pub mod peer;
pub mod run;
pub mod serve;

/// The kinds of things a project leaves, in the order the owner reads them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Group {
    ProjectFiles,
    SessionHistory,
    GitCache,
    Worktrees,
    SavedWork,
}

impl Group {
    pub const ALL: [Group; 5] = [
        Group::ProjectFiles,
        Group::SessionHistory,
        Group::GitCache,
        Group::Worktrees,
        Group::SavedWork,
    ];

    /// The folder it is filed under in the trash folder.
    pub fn dir(self) -> &'static str {
        match self {
            Group::ProjectFiles => "project-files",
            Group::SessionHistory => "session-history",
            Group::GitCache => "git-cache",
            Group::Worktrees => "worktrees",
            Group::SavedWork => "saved-work",
        }
    }

    /// What the owner calls it.
    pub fn label(self) -> &'static str {
        match self {
            Group::ProjectFiles => "project files",
            Group::SessionHistory => "bot session history",
            Group::GitCache => "git cache",
            Group::Worktrees => "worktrees",
            Group::SavedWork => "saved work",
        }
    }
}

/// How an item is held to its place, checked again right before it moves.
#[derive(Debug, Clone)]
pub enum Scope {
    /// Inside `root`, as [`crate::trash::confine::check`] says.
    Under {
        root: PathBuf,
        depth: crate::trash::confine::Depth,
    },
    /// A bot's linked worktree, held to that bot's roots
    /// ([`crate::cleanup::scope::Roots`]) and pruned from its main clone.
    Worktree {
        workspace: Option<PathBuf>,
        bot_name: String,
        main_clone: PathBuf,
    },
}

/// One thing to move to the trash.
#[derive(Debug, Clone)]
pub struct Item {
    pub group: Group,
    pub path: PathBuf,
    pub bytes: u64,
    pub scope: Scope,
}

/// Something found that may not be touched, and why.
#[derive(Debug, Clone, Serialize)]
pub struct Refused {
    pub path: String,
    pub reason: String,
}

/// Everything a project left on this computer.
#[derive(Debug, Clone)]
pub struct Leftovers {
    pub project: Project,
    pub items: Vec<Item>,
    pub refused: Vec<Refused>,
}

impl Leftovers {
    pub fn total_bytes(&self) -> u64 {
        self.items.iter().map(|i| i.bytes).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// The bytes in each group that has any, in the owner's order.
    pub fn by_group(&self) -> Vec<(Group, u64, usize)> {
        Group::ALL
            .iter()
            .filter_map(|group| {
                let items: Vec<&Item> = self.items.iter().filter(|i| i.group == *group).collect();
                (!items.is_empty())
                    .then(|| (*group, items.iter().map(|i| i.bytes).sum(), items.len()))
            })
            .collect()
    }

    /// The preview the owner reads before confirming.
    pub fn view(&self) -> Value {
        let groups: Vec<Value> = self
            .by_group()
            .into_iter()
            .map(|(group, bytes, count)| {
                let items: Vec<Value> = self
                    .items
                    .iter()
                    .filter(|i| i.group == group)
                    .map(|i| json!({ "path": i.path.display().to_string(), "bytes": i.bytes }))
                    .collect();
                json!({
                    "group": group, "label": group.label(), "bytes": bytes,
                    "count": count, "items": items,
                })
            })
            .collect();
        json!({
            "project_id": self.project.id,
            "project_name": crate::db::Db::display_project_name(&self.project),
            "deleted_at": self.project.deleted_at,
            "groups": groups,
            "total_bytes": self.total_bytes(),
            "refused": self.refused,
        })
    }
}

/// Runs `work` off the caller's thread, on the runtime's blocking pool when
/// there is one, its panic contained and logged under `kind`.
pub(crate) fn in_background(kind: &'static str, work: impl FnOnce() + Send + 'static) {
    let contained = move || crate::contain::run(kind, work, || ());
    match tokio::runtime::Handle::try_current() {
        Ok(runtime) => {
            runtime.spawn_blocking(contained);
        }
        Err(_) => {
            std::thread::spawn(contained);
        }
    }
}

/// "1.6 GB", "380 MB", "4 KB": sizes as the owner reads them, decimal.
pub fn size_text(bytes: u64) -> String {
    // Display only: the precision lost converting is far below a KB.
    let b = bytes as f64;
    if b >= 1e9 {
        format!("{:.1} GB", b / 1e9)
    } else if b >= 1e6 {
        format!("{:.0} MB", b / 1e6)
    } else {
        format!("{:.0} KB", b / 1e3)
    }
}

/// "project files 380 MB, bot session history 1.2 GB".
pub fn groups_text(groups: &[(Group, u64)]) -> String {
    groups
        .iter()
        .map(|(group, bytes)| format!("{} {}", group.label(), size_text(*bytes)))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests;
