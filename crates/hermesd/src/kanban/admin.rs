//! Who may change a board's structure, and how (0.19.0 K4).
//!
//! - The owner changes anything, directly.
//! - The lead changes a board directly when its policy is `lead_free`, or
//!   when the change is cosmetic (a name, a description, a column's name,
//!   order, visibility or WIP limit). Otherwise, under `owner_confirms`, the
//!   change is proposed: kept, and raised as a decision; the owner's
//!   `board_proposal_rule` applies or drops it.
//! - Other bots don't change structure.
//! - Owner-only: the structure policy, the default board, archiving.
//!
//! Every lead change is recorded (`board_proposal`), applied or not, for the
//! board's "changed by the lead" list; the lead hears about the owner's.

use std::sync::Arc;

use bus::contract::board as c;
use bus::{now, Bot, DecisionKind, MessageKind, Priority, Ruling};

use super::feed::{Change, ChangeKind};
use super::model::{Role, StructurePolicy};
use crate::actor::Actor;
use crate::app::AppState;
use crate::decisions::publish::publish_settled;
use crate::decisions::service::{raise, RaiseRequest};
use crate::decisions::{conflict, forbidden, invalid, not_found};
use crate::messaging::{self, daemon_sender, Dm};

/// One structure request, as the wire carries it.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Structure {
    Create(c::BoardCreate),
    Board(c::BoardUpdate),
    ColumnAdd(c::ColumnAdd),
    ColumnUpdate(c::ColumnUpdate),
    ColumnRemove(c::ColumnRemove),
    Automation(c::AutomationSet),
}

impl Structure {
    /// The board it changes; empty for a new board.
    pub fn board_id(&self) -> &str {
        match self {
            Structure::Create(_) => "",
            Structure::Board(r) => &r.board_id,
            Structure::ColumnAdd(r) => &r.board_id,
            Structure::ColumnUpdate(r) => &r.board_id,
            Structure::ColumnRemove(r) => &r.board_id,
            Structure::Automation(r) => &r.board_id,
        }
    }

    /// Changes the lead makes directly whatever the policy.
    fn cosmetic(&self) -> bool {
        match self {
            Structure::Board(r) => {
                r.is_default.is_none() && r.archived.is_none() && r.structure_policy.is_none()
            }
            Structure::ColumnUpdate(r) => r.kind.is_none() && r.set_policy != Some(true),
            _ => false,
        }
    }

    fn owner_only(&self) -> bool {
        matches!(self, Structure::Board(r)
            if r.is_default.is_some() || r.archived.is_some() || r.structure_policy.is_some())
    }

    /// One line on what it does, for the decision and the change list.
    pub fn summary(&self) -> String {
        match self {
            Structure::Create(r) => format!("create board “{}” ({})", r.name, r.preset),
            Structure::Board(r) => match &r.name {
                Some(name) => format!("rename the board to “{name}”"),
                None => "change the board's settings".into(),
            },
            Structure::ColumnAdd(r) => {
                let rules = if r.rules.is_empty() {
                    String::new()
                } else {
                    format!(" with rules {}", r.rules.join(", "))
                };
                format!("add column “{}”{rules}", r.name)
            }
            Structure::ColumnUpdate(r) if r.set_policy == Some(true) => format!(
                "set {}'s rules to [{}], movers [{}]",
                r.key,
                r.rules.join(", "),
                r.movers.join(", ")
            ),
            Structure::ColumnUpdate(r) => format!("change column {}", r.key),
            Structure::ColumnRemove(r) => format!("remove column {}", r.key),
            Structure::Automation(r) => match &r.column_key {
                Some(col) if !col.is_empty() => format!("move items to {col} on {}", r.fact),
                _ => format!("stop moving items on {}", r.fact),
            },
        }
    }
}

/// Who asks.
pub enum By<'a> {
    Owner(Actor<'a>),
    Bot { bot: &'a Bot, roles: &'a [Role] },
}

#[derive(Debug, PartialEq)]
pub enum Outcome {
    Applied {
        board_id: String,
    },
    Proposed {
        board_id: String,
        proposal_id: String,
    },
}

pub fn structure(app: &Arc<AppState>, by: By<'_>, req: Structure) -> anyhow::Result<Outcome> {
    // A new board follows the policy of the project's default board.
    let board = match &req {
        Structure::Create(r) => app.db.default_board(&r.project_id)?,
        _ => app.db.board(req.board_id())?,
    }
    .ok_or_else(|| not_found(format!("no board {}", req.board_id())))?;
    let board_id = board.id.clone();
    let (bot, roles) = match by {
        By::Owner(actor) => {
            let board_id = apply(app, &board.project_id, &req, &actor)?;
            tell_lead(app, &board.project_id, None, &format!(
                "The owner changed board {}: {}. Read it again with board_get before you move items.",
                board.name,
                req.summary()
            ))?;
            return Ok(Outcome::Applied { board_id });
        }
        By::Bot { bot, roles } => (bot, roles),
    };
    if bot.project_id != board.project_id {
        return Err(forbidden("that board belongs to another project"));
    }
    if !roles.contains(&Role::Lead) {
        return Err(forbidden(
            "only the lead or the owner changes a board's structure; ask the lead",
        ));
    }
    if req.owner_only() {
        return Err(forbidden(
            "the structure policy, the default board and archiving are the owner's",
        ));
    }
    let request = serde_json::to_string(&req)?;
    let summary = req.summary();
    let by_lead = format!("bot:{}", bot.id);
    if board.structure_policy == StructurePolicy::LeadFree || req.cosmetic() {
        let board_id = apply(
            app,
            &board.project_id,
            &req,
            &Actor::Bot {
                id: &bot.id,
                project_id: &bot.project_id,
            },
        )?;
        app.db.add_board_proposal(
            &board.project_id,
            Some(&board_id),
            &request,
            &summary,
            &by_lead,
            "applied",
        )?;
        return Ok(Outcome::Applied { board_id });
    }
    // Checked now, so the owner is never asked to approve what can't apply.
    if !matches!(req, Structure::Create(_)) {
        app.db
            .board_read(|t| t.board_at(&board_id, version_of(&req)).map(|_| ()))?;
    }
    let proposal = app.db.add_board_proposal(
        &board.project_id,
        Some(&board_id),
        &request,
        &summary,
        &by_lead,
        "open",
    )?;
    let title = format!("Board change proposed for {}: {summary}", board.name);
    let body = format!(
        "{} proposes to {summary} on board {}.\n\nApprove or decline it in Needs you; \
         nothing changes until you do. Proposal {}.",
        bot.name, board.name, proposal.id
    );
    let raised = raise(
        app,
        bot,
        &RaiseRequest {
            kind: DecisionKind::Question,
            title: &title,
            body: &body,
            options: &[],
            recommendation: None,
            tags: &[],
            priority: Priority::Normal,
            deadline_at: None,
            on_behalf_of: None,
            source_task_id: None,
            supersedes: None,
        },
    )?;
    app.db
        .set_proposal_decision(&proposal.id, &raised.decision.id)?;
    Ok(Outcome::Proposed {
        board_id,
        proposal_id: proposal.id,
    })
}

/// The owner's ruling on a proposal: approved, it is applied as proposed
/// (refused, and dropped as stale, when the board changed since); declined,
/// it is dropped. Either settles its decision, which tells the lead.
pub fn rule(
    app: &Arc<AppState>,
    actor: &Actor<'_>,
    proposal_id: &str,
    approve: bool,
    note: Option<&str>,
) -> anyhow::Result<Outcome> {
    let proposal = app
        .db
        .board_proposal(proposal_id)?
        .ok_or_else(|| not_found(format!("no proposal {proposal_id}")))?;
    if proposal.state != "open" {
        return Err(conflict(format!(
            "that proposal is already {}",
            proposal.state
        )));
    }
    let req: Structure = serde_json::from_str(&proposal.request)?;
    let (state, text, result) = if approve {
        match apply(app, &proposal.project_id, &req, actor) {
            Ok(id) => (
                "applied",
                format!("Approved: {}.", proposal.summary),
                Ok(id),
            ),
            Err(e) => (
                "stale",
                format!("Not applied: {e}. Propose it again on the board as it is now."),
                Err(e),
            ),
        }
    } else {
        let id = proposal.board_id.clone().unwrap_or_default();
        (
            "declined",
            format!("Declined: {}.", proposal.summary),
            Ok(id),
        )
    };
    let text = match note.map(str::trim).filter(|n| !n.is_empty()) {
        Some(note) => format!("{text} {note}"),
        None => text,
    };
    if !app.db.rule_board_proposal(proposal_id, state, note)? {
        return Err(conflict("that proposal was ruled on meanwhile"));
    }
    if let Some(decision_id) = &proposal.decision_id {
        let ruling = Ruling {
            option: None,
            text,
            reason: None,
            answered_at: now(),
            answered_by: match actor {
                Actor::Device(id) => format!("device:{id}"),
                _ => "owner".to_string(),
            },
        };
        if app.db.try_answer(decision_id, &ruling)? {
            publish_settled(app, decision_id, None)?;
        }
    }
    Ok(Outcome::Applied { board_id: result? })
}

fn version_of(req: &Structure) -> u64 {
    match req {
        Structure::Create(_) => 0,
        Structure::Board(r) => r.version,
        Structure::ColumnAdd(r) => r.version,
        Structure::ColumnUpdate(r) => r.version,
        Structure::ColumnRemove(r) => r.version,
        Structure::Automation(r) => r.version,
    }
}

/// Applies the change; returns the board it changed (or made).
fn apply(
    app: &Arc<AppState>,
    project_id: &str,
    req: &Structure,
    actor: &Actor<'_>,
) -> anyhow::Result<String> {
    let mut feed = app.board.writer();
    let board_id = match req {
        Structure::Create(r) => {
            let preset = super::presets::Preset::parse(&r.preset)
                .ok_or_else(|| invalid(format!("“{}” is not a preset", r.preset)))?;
            validate_new(&r.name)?;
            let key = r
                .key
                .clone()
                .filter(|k| !k.trim().is_empty())
                .unwrap_or_else(|| super::defaults::key_for(&r.name));
            let board = app
                .db
                .create_board(project_id, r.name.trim(), key.trim(), preset)?;
            if let Some(description) = r.description.as_deref().filter(|d| !d.is_empty()) {
                app.db.board_tx(|t| {
                    t.update_board(&c::BoardUpdate {
                        board_id: board.id.clone(),
                        version: board.version,
                        description: Some(description.to_string()),
                        ..Default::default()
                    })
                })?;
            }
            board.id
        }
        _ => {
            app.db.board_tx(|t| match req {
                Structure::Board(r) => t.update_board(r),
                Structure::ColumnAdd(r) => t.add_column(r).map(|_| ()),
                Structure::ColumnUpdate(r) => t.update_column(r),
                Structure::ColumnRemove(r) => t.remove_column(r, actor),
                Structure::Automation(r) => t.set_automation(r),
                Structure::Create(_) => unreachable!("handled above"),
            })?;
            req.board_id().to_string()
        }
    };
    let kind = match req {
        Structure::Create(_) | Structure::Board(_) | Structure::Automation(_) => {
            ChangeKind::SettingsChanged
        }
        _ => ChangeKind::ColumnsChanged,
    };
    feed.publish(Change {
        project_id,
        kind,
        item_id: "",
        card: None,
        from_column: None,
    });
    Ok(board_id)
}

/// A note to the project's lead, unless the lead made the change.
pub(super) fn tell_lead(
    app: &AppState,
    project_id: &str,
    by: Option<&str>,
    body: &str,
) -> anyhow::Result<()> {
    let Some(project) = app.db.get_project(project_id)? else {
        return Ok(());
    };
    let lead = match project.lead_bot_id {
        Some(lead) => Some(lead),
        None => app
            .db
            .project_roles(project_id)?
            .into_iter()
            .find(|r| r.role == Role::Lead)
            .map(|r| r.bot_id),
    };
    let Some(lead) = lead.filter(|l| Some(l.as_str()) != by) else {
        return Ok(());
    };
    messaging::send_dm(
        &app.db,
        &app.events,
        Dm::new(&lead, &daemon_sender(), MessageKind::Note, body),
    )?;
    Ok(())
}

/// A board name and key are given; the key defaults to the name's initials.
pub fn validate_new(name: &str) -> anyhow::Result<()> {
    anyhow::ensure!(!name.trim().is_empty(), invalid("a board needs a name"));
    Ok(())
}
