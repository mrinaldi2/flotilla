//! A project near or at its weekly token budget (0.24.0, `usage_v1`): a
//! `budget_warning` row from 80%, and a `budget_reached` row once it is
//! used up and not extended, while its on-demand bots are held. Each
//! computer counts its own bots, so each raises its own.

use bus::contract::home::AttentionKind;

use super::rows::{Builder, Part};
use super::weight;
use crate::app::AppState;
use crate::db::Db;
use crate::usage::budget;

/// From this share of the budget, in percent, the owner is warned.
const WARN_AT: u64 = 80;

pub(super) fn rows(app: &AppState, b: &mut Builder<'_>) -> anyhow::Result<()> {
    let Some(state) = budget::of(app, b.project_id)? else {
        return Ok(());
    };
    let percent = state.percent();
    if !state.reached && percent < WARN_AT {
        return Ok(());
    }
    let name = app
        .db
        .get_project(b.project_id)?
        .map(|p| Db::display_project_name(&p))
        .unwrap_or_default();
    let (kind, title) = if state.reached {
        (
            AttentionKind::BudgetReached,
            format!("{name} used its weekly budget: its on-demand bots are held"),
        )
    } else {
        (
            AttentionKind::BudgetWarning,
            format!("{name} used {percent}% of its weekly budget"),
        )
    };
    let part = Part {
        kind,
        target_id: b.project_id.to_string(),
        title,
        // Stable for the day, so the row keeps its place across reads.
        created_at: budget::midnight(budget::today()),
        target: None,
    };
    b.push(part, weight(kind), None);
    Ok(())
}
