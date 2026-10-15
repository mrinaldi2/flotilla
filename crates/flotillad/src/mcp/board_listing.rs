//! Which roles `tools/list` shows a bot's board tools for (moved out of
//! board.rs for the 400-line limit).

use std::sync::Arc;

use crate::app::AppState;
use crate::kanban::model::Role;

use super::board::board_roles;

/// The roles `tools/list` shows a bot's tools for: its board roles; on a
/// linked computer, the roles the board's home gives it, as mirrored here,
/// since its calls go there (`board_remote`); or before its project has a
/// board, the lead's role for the project's lead, which the board gives it.
/// A running session keeps the list it read at its start, so the lead
/// already holds its tools when the owner starts the board.
pub(super) fn listed_roles(app: &Arc<AppState>, bot: &bus::Bot) -> anyhow::Result<Vec<Role>> {
    if let Some(roles) = board_roles(app, bot)? {
        return Ok(roles);
    }
    if let Some(home) = app.board_mirror.get(&bot.project_id) {
        // Only for listing: the home checks its own roles on every call.
        return Ok(home
            .snapshot
            .roles
            .iter()
            .filter(|r| r.bot_id == bot.id)
            .filter_map(|r| Role::from_wire(r.role).ok())
            .collect());
    }
    let is_lead = app
        .db
        .get_project(&bot.project_id)?
        .is_some_and(|p| p.lead_bot_id.as_deref() == Some(bot.id.as_str()));
    Ok(is_lead.then_some(Role::Lead).into_iter().collect())
}
