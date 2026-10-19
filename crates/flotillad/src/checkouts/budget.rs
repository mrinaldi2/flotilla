//! A project's disk budget (0.23.0, `set_project_disk_budget`): what its
//! bots' workspaces, worktrees and build caches, its card worktrees and its
//! mirror may use on this computer. The hourly disk report counts each
//! project that has one (`budgets` in the report). Over budget, it first
//! removes the worktrees of merged cards, under every cleanup rule; what is
//! still over raises the project's Needs-you row.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use super::mirror::Mirror;
use crate::app::AppState;
use crate::cleanup::model::Outcome;
use crate::cleanup::remove::size_of;
use crate::cleanup::run::{self, Merge, Target};
use crate::db::CardWorktree;
use crate::prs::model::PrState;

/// The projects with a budget, and what each uses here. `uses` are the
/// report's per-bot rows, already sized.
pub fn usage(app: &AppState, uses: &[Value]) -> Vec<Value> {
    let mut out = Vec::new();
    for project in app.db.list_projects().unwrap_or_default() {
        let Some(budget) = app.db.project_disk_budget(&project.id).ok().flatten() else {
            continue;
        };
        let bots: Vec<String> = app
            .db
            .list_bots(Some(&project.id))
            .unwrap_or_default()
            .into_iter()
            .map(|b| b.id)
            .collect();
        let of_bots: u64 = uses
            .iter()
            .filter(|u| {
                u["bot"]["id"]
                    .as_str()
                    .is_some_and(|id| bots.iter().any(|b| b == id))
            })
            .map(|u| {
                ["workspace_bytes", "worktree_bytes", "cache_bytes"]
                    .iter()
                    .map(|k| u[*k].as_u64().unwrap_or(0))
                    .sum::<u64>()
            })
            .sum();
        let cards: u64 = app
            .db
            .card_worktrees(Some(&project.id))
            .unwrap_or_default()
            .iter()
            .map(|t| size_of(Path::new(&t.path)))
            .sum();
        let mirror = Mirror::of_project(app, &project.id)
            .map(|m| size_of(&m.path))
            .unwrap_or(0);
        out.push(json!({
            "project_id": project.id, "budget_bytes": budget,
            "used_bytes": of_bots + cards + mirror, "card_worktree_bytes": cards,
        }));
    }
    out
}

/// The projects in `report` over their budget: id, budget, used.
pub fn over(report: &Value) -> Vec<(String, u64, u64)> {
    report["budgets"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|b| {
            let (budget, used) = (b["budget_bytes"].as_u64()?, b["used_bytes"].as_u64()?);
            let project = b["project_id"].as_str()?;
            (used > budget).then(|| (project.to_string(), budget, used))
        })
        .collect()
}

/// The project's card worktrees whose card has a merged PR, oldest first,
/// each with its merged commit.
fn merged_cards(app: &AppState, project_id: &str) -> Vec<(CardWorktree, String)> {
    let merged = app
        .db
        .board_read(|t| t.prs(project_id, &[PrState::Merged]))
        .unwrap_or_default();
    app.db
        .card_worktrees(Some(project_id))
        .unwrap_or_default()
        .into_iter()
        .filter_map(|tree| {
            let pr = merged.iter().find(|p| p.item_id == tree.item_id)?;
            let sha = pr.merged_sha.clone()?;
            Some((tree, sha))
        })
        .collect()
}

/// Removes merged cards' worktrees of each project `report` finds over
/// budget until it is under. The bytes freed.
pub fn enforce(app: &AppState, report: &Value) -> u64 {
    let mut freed = 0;
    for (project_id, budget, mut used) in over(report) {
        let mirror = Mirror::of_project(app, &project_id).ok();
        for (tree, sha) in merged_cards(app, &project_id) {
            if used <= budget {
                break;
            }
            let bot = app.db.get_bot(&tree.bot_id).ok().flatten();
            let target = Target {
                path: PathBuf::from(&tree.path),
                main_clone: mirror.as_ref().map(|m| m.path.clone()),
                bot_name: bot.as_ref().map(|b| b.name.clone()).unwrap_or_default(),
                workspace: bot
                    .map(|b| PathBuf::from(b.workspace_path))
                    .filter(|w| w.is_absolute()),
                no_other_work: false,
            };
            let merge = Merge {
                merged_sha: &sha,
                salvage: super::tidy::salvage_dir(app, &tree),
            };
            match run::attempt(app, &target, &merge) {
                Outcome::Done { bytes } => {
                    let _ = app.db.remove_card_worktree(&tree.project_id, &tree.item_id);
                    used = used.saturating_sub(bytes);
                    freed += bytes;
                    tracing::info!(path = %tree.path, bytes, "over budget: a merged card's worktree went");
                }
                outcome => {
                    tracing::info!(path = %tree.path, ?outcome, "over budget: a merged card's worktree stays");
                }
            }
        }
    }
    freed
}

/// The hourly report: taken, the budgets enforced, taken again if that
/// freed anything, and kept.
pub fn hourly(app: &AppState) -> anyhow::Result<Value> {
    let report = crate::cleanup::disk::take(app)?;
    let report = match enforce(app, &report) {
        0 => report,
        _ => crate::cleanup::disk::take(app)?,
    };
    crate::cleanup::disk::keep(app, &report)?;
    Ok(report)
}
