//! Assigning a card to a role (0.23.0, `team_on_demand_v1`): the lead names
//! the role that owns the card's next step, and the daemon picks which of
//! its holders gets it. The assignment notice then reaches that bot the
//! usual way, which wakes it when it sleeps.

use std::collections::HashSet;

use bus::contract::board as c;
use bus::BotState;

use super::guards::unmet;
use super::model::{ColumnKind, Role, Unmet};
use crate::app::AppState;

/// How ready a holder is to take the card, best first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Readiness {
    /// Running, idle, with no open task and no other card in an active
    /// column.
    Free,
    /// Asleep, or woken and waiting for a running slot: the notice wakes it.
    Asleep,
    /// Busy, stopped, or on another computer: its work waits.
    Other,
}

/// The role `item_assign` names, if any; `ROLE_UNSPECIFIED` is none.
pub fn requested(req: &c::ItemAssign) -> anyhow::Result<Option<Role>> {
    req.role
        .filter(|r| *r != c::Role::Unspecified as i32)
        .map(Role::from_wire)
        .transpose()
        .map_err(Into::into)
}

/// The holder of `role` the card goes to: one that is awake and free, else
/// one that is asleep or queued, else the first. Holders are taken in name
/// order, so the pick is the same every time. `None` when nobody holds it.
pub fn holder(
    app: &AppState,
    project_id: &str,
    role: Role,
    item_id: &str,
) -> anyhow::Result<Option<String>> {
    let held: HashSet<String> = app
        .db
        .project_roles(project_id)?
        .into_iter()
        .filter(|r| r.role == role)
        .map(|r| r.bot_id)
        .collect();
    let mut holders: Vec<bus::Bot> = app
        .db
        .list_bots(Some(project_id))?
        .into_iter()
        .filter(|b| held.contains(&b.id))
        .collect();
    holders.sort_by(|a, b| (a.name.to_lowercase(), &a.id).cmp(&(b.name.to_lowercase(), &b.id)));
    if holders.is_empty() {
        return Ok(None);
    }
    let active = active_assignees(app, project_id, item_id)?;
    let mut best: Option<(Readiness, &bus::Bot)> = None;
    for bot in &holders {
        let readiness = readiness(app, bot, &active)?;
        if best.is_none_or(|(r, _)| readiness < r) {
            best = Some((readiness, bot));
        }
    }
    Ok(best.map(|(_, bot)| bot.id.clone()))
}

/// Why a role nobody holds can't take a card.
pub fn unheld(role: Role) -> Unmet {
    unmet(
        "role.unheld",
        format!("No bot holds the {} role.", role.as_str()),
        Some("Give the role to a bot first (role_set), or assign a bot by name."),
    )
}

fn readiness(
    app: &AppState,
    bot: &bus::Bot,
    active: &HashSet<String>,
) -> anyhow::Result<Readiness> {
    if bot.is_linked() {
        return Ok(Readiness::Other);
    }
    Ok(match app.supervisor.state(&bot.id).0 {
        BotState::Asleep | BotState::Queued => Readiness::Asleep,
        BotState::Ready
            if !active.contains(&bot.id) && app.db.open_tasks_for(&bot.id)?.is_empty() =>
        {
            Readiness::Free
        }
        _ => Readiness::Other,
    })
}

/// The bots holding a card in an active column, this card aside: being
/// assigned the card again doesn't make its holder busy.
fn active_assignees(
    app: &AppState,
    project_id: &str,
    item_id: &str,
) -> anyhow::Result<HashSet<String>> {
    let active: HashSet<(String, String)> = app
        .db
        .board_columns(project_id)?
        .into_iter()
        .filter(|c| c.kind == ColumnKind::Active)
        .map(|c| (c.board_id, c.key))
        .collect();
    Ok(app
        .db
        .board_cards(project_id)?
        .into_iter()
        .filter(|card| card.id != item_id && !card.draft)
        .filter(|card| active.contains(&(card.board_id.clone(), card.column_key.clone())))
        .filter_map(|card| card.assignee)
        .collect())
}
