//! A project's roles (0.19.0 K8): who holds which role, set by the owner in
//! the app and by the lead over MCP (`role_set`). Roles belong to the project,
//! not to Kanban: boards read them, and releases add their own rule on who
//! tests a device. Each change is pushed, so open boards refetch.

use std::sync::Arc;

use bus::contract::board as c;

use crate::app::AppState;
use crate::kanban::edit::project_bot;
use crate::kanban::feed::{Change, ChangeKind};
use crate::kanban::model::{ProjectRole, Role};
use crate::releases::machines::IOS_TEST;

/// The roles the lead may give or take away over MCP: the team's working
/// roles, Product and Architect (0.23.0) as Dev. Lead, DevOps and tester are
/// the owner's alone (ARCH-R30 M1), and so is reviewer.ce: the security
/// review is independent of the lead.
const LEAD_ASSIGNS: [Role; 6] = [
    Role::Dev,
    Role::Product,
    Role::Architect,
    Role::Coach,
    Role::ReviewerArch,
    Role::ReviewerUx,
];

/// Who assigns a role.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Assigner<'a> {
    /// The owner on the WebSocket; `proved` when from a paired device or the
    /// app's ticket rather than the owner token a bot could read.
    Owner { proved: bool },
    /// The lead over MCP, by bot id.
    Lead(&'a str),
}

/// Gives the bot the role, or takes it away. A tester's machine is updated
/// in place. The owner assigns any role (reviewer.ce only from a device or
/// the app); the lead only `LEAD_ASSIGNS`, and never a reviewer role to
/// itself.
pub fn set_role(
    app: &Arc<AppState>,
    project_id: &str,
    req: &c::RoleSet,
    by: Assigner<'_>,
) -> anyhow::Result<()> {
    let role = Role::from_wire(req.role)?;
    let bot_id = project_bot(app, project_id, req.bot.trim())?;
    let by_owner = matches!(by, Assigner::Owner { .. });
    anyhow::ensure!(
        role != Role::ReviewerCe || by == Assigner::Owner { proved: true },
        "only the owner, from the app or a paired device, assigns reviewer.ce"
    );
    let reviewer = matches!(
        role,
        Role::ReviewerArch | Role::ReviewerUx | Role::ReviewerCe
    );
    anyhow::ensure!(
        !(reviewer && by == Assigner::Lead(&bot_id)),
        "the lead can't give itself a reviewer role"
    );
    anyhow::ensure!(
        by_owner || LEAD_ASSIGNS.contains(&role) || lead_ios_tester(app, project_id, req, &bot_id)?,
        "only the owner assigns {}",
        role.as_str()
    );
    // The other order of that separation: the lead can't make its iOS
    // tester a bot that leads, ships or builds either.
    anyhow::ensure!(
        by_owner
            || req.remove == Some(true)
            || !matches!(role, Role::Lead | Role::Devops | Role::Dev)
            || !tests_ios(app, project_id, &bot_id)?,
        "only the owner makes the iOS tester a {}",
        role.as_str()
    );
    anyhow::ensure!(
        app.db.board_settings(project_id)?.is_some(),
        "this project has no board yet"
    );
    let mut feed = app.board.writer();
    if req.remove == Some(true) {
        app.db.remove_project_role(project_id, role, &bot_id)?;
    } else {
        let machine = req
            .machine
            .as_deref()
            .map(str::trim)
            .filter(|m| !m.is_empty());
        anyhow::ensure!(
            machine.is_none() || role == Role::Tester,
            "only a tester has a machine"
        );
        app.db.set_project_role(&ProjectRole {
            project_id: project_id.to_string(),
            role,
            bot_id: bot_id.clone(),
            machine: machine.map(str::to_string),
        })?;
    }
    feed.publish(Change {
        project_id,
        kind: ChangeKind::SettingsChanged,
        item_id: "",
        card: None,
        from_column: None,
    });
    if req.remove != Some(true) {
        crate::kanban::on_entry::role_held(app, &mut feed, project_id, role);
    }
    drop(feed);
    // Its prompt says whether it has a browser, which its roles decide.
    if let Some(bot) = app.db.get_live_bot(&bot_id)? {
        if let Err(error) = crate::botmgmt::reprovision(app, &bot) {
            tracing::warn!(bot_id, %error, "system.md not rewritten for the role");
        }
    }
    Ok(())
}

/// The one tester role the lead may give or take away: a tester on
/// `ios`, who records iOS packages' results and carries the iPhone deploy.
/// It reaches no desktop build, so it stays out of ARCH-R30's owner-only
/// set. Only on `ios` itself (not the iPhone), and never for a bot that
/// leads, ships or builds (the lead itself, DevOps, a dev): those keep the
/// separation of duties the owner's rule protects. A bot that already tests
/// a desktop computer isn't moved off it.
fn lead_ios_tester(
    app: &AppState,
    project_id: &str,
    req: &c::RoleSet,
    bot_id: &str,
) -> anyhow::Result<bool> {
    if Role::from_wire(req.role)? != Role::Tester {
        return Ok(false);
    }
    let lead = app.db.get_project(project_id)?.and_then(|p| p.lead_bot_id);
    let roles: Vec<_> = app
        .db
        .project_roles(project_id)?
        .into_iter()
        .filter(|r| r.bot_id == bot_id)
        .collect();
    let separated = lead.as_deref() != Some(bot_id)
        && !roles
            .iter()
            .any(|r| matches!(r.role, Role::Lead | Role::Devops | Role::Dev));
    let current = roles.iter().find(|r| r.role == Role::Tester);
    if req.remove == Some(true) {
        return Ok(separated && current.is_some_and(|r| on_ios(r.machine.as_deref())));
    }
    Ok(separated
        && on_ios(req.machine.as_deref())
        && current.is_none_or(|r| on_ios(r.machine.as_deref())))
}

fn on_ios(machine: Option<&str>) -> bool {
    machine.is_some_and(|m| m.trim().eq_ignore_ascii_case(IOS_TEST))
}

/// Whether the bot is a tester on `ios`.
fn tests_ios(app: &AppState, project_id: &str, bot_id: &str) -> anyhow::Result<bool> {
    Ok(app
        .db
        .project_roles(project_id)?
        .iter()
        .any(|r| r.bot_id == bot_id && r.role == Role::Tester && on_ios(r.machine.as_deref())))
}
