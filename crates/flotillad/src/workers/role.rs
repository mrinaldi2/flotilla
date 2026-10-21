//! What a worker starts with: its instructions, and for a worker from a
//! role (0.23.0, `team_on_demand_v1`) what it copies. Workers from a role
//! are extra hands for a role whose cards can be worked at once. Each is a
//! copy of the role's member, taking its instructions and pinned model, and
//! holds the role on the board until it retires, when archiving it drops
//! the role.
//!
//! A spawn may wait in the queue, and the worker table has no room for a
//! role, so what the worker copies is kept in `meta` under its spawn's id
//! until it starts.

use bus::{Bot, Worker};
use serde::{Deserialize, Serialize};

use crate::app::AppState;
use crate::kanban::feed::{Change, ChangeKind};
use crate::kanban::model::{ProjectRole, Role};

/// Instructions for a worker spawned without any. Non-empty so the charter
/// for a bot "created from a name alone" — which tells it to ask what it is
/// for — never applies: a worker's task is its whole charter.
const DEFAULT_INSTRUCTIONS: &str =
    "Do the one task you are given and report it with complete_task.";

/// The worker's instructions, or the default.
pub(super) fn instructions(worker: &Worker) -> &str {
    if worker.instructions.trim().is_empty() {
        DEFAULT_INSTRUCTIONS
    } else {
        &worker.instructions
    }
}

/// What a worker from a role takes from the role's member.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoleCopy {
    pub role: Role,
    /// The member's pinned model, when it has one.
    pub model: Option<String>,
}

/// A spawn from `role`: only the project's lead may ask for one. Returns the
/// worker's instructions (the member's, then any given) and what it copies.
pub(super) fn from_member(
    app: &AppState,
    parent: &Bot,
    role: Role,
    given: Option<&str>,
) -> anyhow::Result<(String, RoleCopy)> {
    let lead = app
        .db
        .get_project(&parent.project_id)?
        .and_then(|p| p.lead_bot_id);
    anyhow::ensure!(
        lead.as_deref() == Some(parent.id.as_str()),
        "only the lead spawns a worker from a role; spawn one without 'role'"
    );
    // The owner's own roles stay one bot each: the lead, DevOps and the
    // security reviewer.
    anyhow::ensure!(
        !matches!(role, Role::Lead | Role::Devops | Role::ReviewerCe),
        "a worker can't copy the {} role",
        role.as_str()
    );
    let member = member(app, &parent.project_id, role)?.ok_or_else(|| {
        anyhow::anyhow!(
            "no bot holds the {} role to copy; give it to one first (role_set)",
            role.as_str()
        )
    })?;
    let given = given.map(str::trim).filter(|g| !g.is_empty());
    let instructions = match (member.instructions.trim(), given) {
        (own, Some(given)) if !own.is_empty() => format!("{own}\n\n{given}"),
        ("", Some(given)) => given.to_string(),
        (own, _) => own.to_string(),
    };
    let model = app.db.bot_model(&member.id)?;
    Ok((instructions, RoleCopy { role, model }))
}

/// The role's member: the first of its permanent holders here, by name.
fn member(app: &AppState, project_id: &str, role: Role) -> anyhow::Result<Option<Bot>> {
    let holders: Vec<String> = app
        .db
        .project_roles(project_id)?
        .into_iter()
        .filter(|r| r.role == role)
        .map(|r| r.bot_id)
        .collect();
    let mut members: Vec<Bot> = app
        .db
        .list_bots(Some(project_id))?
        .into_iter()
        .filter(|b| !b.temporary && !b.is_linked() && holders.contains(&b.id))
        .collect();
    members.sort_by(|a, b| (a.name.to_lowercase(), &a.id).cmp(&(b.name.to_lowercase(), &b.id)));
    Ok(members.into_iter().next())
}

fn key(worker_id: &str) -> String {
    format!("worker_role:{worker_id}")
}

/// Keep what the queued spawn copies until it starts.
pub(super) fn remember(app: &AppState, worker_id: &str, copy: &RoleCopy) -> anyhow::Result<()> {
    app.db
        .set_meta(&key(worker_id), &serde_json::to_string(copy)?)
}

/// What the spawn copies, if it is from a role.
pub(super) fn recall(app: &AppState, worker: &Worker) -> Option<RoleCopy> {
    let raw = app.db.get_meta(&key(&worker.id)).ok().flatten()?;
    serde_json::from_str(&raw).ok()
}

/// Done with the spawn's copy: it started or will never start.
pub(super) fn forget(app: &AppState, worker_id: &str) {
    if let Err(error) = app.db.delete_meta(&key(worker_id)) {
        tracing::warn!(worker_id, %error, "a worker's role copy was not removed");
    }
}

/// Give the worker's bot its role and the member's model, before its first
/// session starts. The model applies to a worker here only: one on a linked
/// computer picks that computer's default.
pub fn apply(app: &AppState, bot: &Bot, copy: &RoleCopy) -> anyhow::Result<()> {
    let mut feed = app.board.writer();
    app.db.set_project_role(&ProjectRole {
        project_id: bot.project_id.clone(),
        role: copy.role,
        bot_id: bot.id.clone(),
        machine: None,
    })?;
    // Open boards refetch the roles.
    feed.publish(Change {
        project_id: &bot.project_id,
        kind: ChangeKind::SettingsChanged,
        item_id: "",
        card: None,
        from_column: None,
    });
    crate::kanban::on_entry::role_held(app, &mut feed, &bot.project_id, copy.role);
    drop(feed);
    if !bot.is_linked() {
        if let Some(model) = &copy.model {
            app.db.set_bot_model(&bot.id, Some(model))?;
        }
    }
    Ok(())
}
