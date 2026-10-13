//! The move engine (H-020 §1.2). `item_move` runs the guards and commits;
//! `item_move_check` runs them for every column and writes nothing. WS, MCP
//! and the peer link all come through here, so each rule lives only in
//! `guards`.

use crate::actor::Actor;
use crate::db::{BoardTx, Db, MoveTo, Write};

use super::flow;
use super::guards::{self, unmet, Move, Rule, Who};
use super::model::{BoardColumn, BoardFlow, Item, Unmet};

/// The note on the move of an item the owner closed out of Verify with no
/// release package.
pub const CLOSED_WITHOUT_RELEASE: &str = "closed by the owner without a release";

/// The note on the close of a spike or a chore without code (H-154),
/// followed by the outcome it closed on.
pub const CLOSED_ON_OUTCOME: &str = "closed on its outcome";

pub struct MoveRequest<'a> {
    pub id: &'a str,
    pub to: &'a str,
    pub expected_version: u64,
    pub reason: Option<&'a str>,
    pub override_reason: Option<&'a str>,
}

#[derive(Debug)]
pub enum Moved {
    Done(Box<Item>),
    /// The guards that are not met; nothing was written.
    Refused(Vec<Unmet>),
    /// The item changed since the caller read it; this is how it is now.
    Conflict(Box<Item>),
}

/// The item, its project and the actor as the guards see it, read inside
/// the caller's transaction. A bot of another project is refused here.
pub(crate) fn load_in(
    t: &BoardTx<'_>,
    id: &str,
    actor: &Actor<'_>,
) -> anyhow::Result<(Item, String, Who)> {
    let missing = || anyhow::anyhow!("item {id} not found");
    let item = t.item(id)?.ok_or_else(missing)?;
    let project_id = t.item_project(id)?.ok_or_else(missing)?;
    let who = match actor {
        Actor::User | Actor::Device(_) => Who::Owner,
        Actor::Bot {
            id: bot,
            project_id: own,
        } => {
            anyhow::ensure!(*own == project_id, "item {id} belongs to another project");
            let roles = t
                .roles(&project_id)?
                .into_iter()
                .filter(|r| r.bot_id == *bot)
                .map(|r| r.role)
                .collect();
            Who::Bot {
                id: bot.to_string(),
                roles,
            }
        }
    };
    Ok((item, project_id, who))
}

/// Move an item if every guard holds, all in one transaction (ARCH-R4 §3).
/// A stale `expected_version` is a conflict before any guard runs, so
/// refusals always describe the current card.
pub fn item_move(db: &Db, req: &MoveRequest<'_>, actor: &Actor<'_>) -> anyhow::Result<Moved> {
    db.board_tx(|t| move_in(t, req, actor))
}

/// [`item_move`] inside the caller's transaction (a draft's finalize).
pub(crate) fn move_in(
    t: &BoardTx<'_>,
    req: &MoveRequest<'_>,
    actor: &Actor<'_>,
) -> anyhow::Result<Moved> {
    let (item, project_id, who) = load_in(t, req.id, actor)?;
    if item.version != req.expected_version {
        return Ok(Moved::Conflict(Box::new(item)));
    }
    if let Some(draft) = super::edit::not_live(&item) {
        return Ok(Moved::Refused(vec![draft]));
    }
    let columns = board_columns(t, &item)?;
    let Some(to) = columns.iter().find(|c| c.key == req.to) else {
        let text = format!("This board has no column {}.", req.to);
        return Ok(Moved::Refused(vec![unmet("move.column", text, None)]));
    };
    let ctx = t.move_context(&project_id, &item)?;
    let mv = Move {
        to,
        reason: req.reason,
        override_reason: req.override_reason,
    };
    let over = guards::over_limit(&item, to, &ctx).is_some() && who != Who::Daemon;
    let (note, first) = if let Some(from) = custom_from(t, &item, &columns)? {
        let refused = flow::evaluate(&item, from, &to.policy, &mv, &who, &ctx);
        if !refused.is_empty() {
            return Ok(Moved::Refused(refused));
        }
        let first = flow::is_return(from, to);
        (custom_note(&mv, over, first), first)
    } else {
        let refused = guards::evaluate(&item, &mv, &who, &ctx);
        if !refused.is_empty() {
            return Ok(Moved::Refused(refused));
        }
        let rule = guards::rule(item.category, to.category);
        let escape = guards::closes_without_release(&item, &mv, &who, &ctx);
        // A spike or a chore closed on its outcome names it in the history
        // (H-154): the newest artifact linked, which the guard required.
        let outcome = (rule == Rule::Finish && !escape)
            .then(|| guards::outcome_link(&ctx))
            .flatten();
        (
            note(rule, &mv, over, escape, outcome),
            guards::is_return(rule),
        )
    };
    Ok(
        match t.move_item(
            req.id,
            req.expected_version,
            &MoveTo {
                column: req.to,
                note: note.as_deref(),
                wip_override: over,
                first,
            },
            actor,
        )? {
            Write::Done(item) => Moved::Done(Box::new(item)),
            Write::Conflict(current) => Moved::Conflict(current),
        },
    )
}

/// What the move's history event says: the reason, the review verdict, any
/// WIP override (automatic for returned work, H-017 rev 2.1 §1.3) and an
/// owner's close without a release (ARCH-R22 F1).
fn note(
    rule: Rule,
    mv: &Move<'_>,
    over: bool,
    escape: bool,
    outcome: Option<&str>,
) -> Option<String> {
    let mut parts = Vec::new();
    if escape {
        parts.push(CLOSED_WITHOUT_RELEASE.to_string());
    }
    if let Some(outcome) = outcome {
        parts.push(format!("{CLOSED_ON_OUTCOME}: {outcome}"));
    }
    if let Some(reason) = mv.reason.map(str::trim).filter(|r| !r.is_empty()) {
        parts.push(reason.to_string());
    }
    if rule == Rule::Approve {
        parts.push("verdict: approve".to_string());
    }
    if over && guards::is_return(rule) {
        parts.push(guards::RETURNED_OVER_WIP.to_string());
    } else if over {
        let reason = mv.override_reason.unwrap_or_default().trim();
        parts.push(format!("WIP override: {reason}"));
    }
    (!parts.is_empty()).then(|| parts.join(" · "))
}

/// The columns of the item's own board (every column of the project for an
/// item from a home older than 0.19, which names no board).
fn board_columns(t: &BoardTx<'_>, item: &Item) -> anyhow::Result<Vec<BoardColumn>> {
    let project_id = t
        .item_project(&item.id)?
        .ok_or_else(|| anyhow::anyhow!("item {} not found", item.id))?;
    Ok(t.columns(&project_id)?
        .into_iter()
        .filter(|c| item.board_id.is_empty() || c.board_id == item.board_id)
        .collect())
}

/// The item's column, when its board is custom; `None` for a release-gated
/// board, whose guards decide.
fn custom_from<'c>(
    t: &BoardTx<'_>,
    item: &Item,
    columns: &'c [BoardColumn],
) -> anyhow::Result<Option<&'c BoardColumn>> {
    let custom = t
        .board(&item.board_id)?
        .is_some_and(|b| b.flow == BoardFlow::Custom);
    Ok(custom
        .then(|| columns.iter().find(|c| c.key == item.column_key))
        .flatten())
}

/// The history note of a move on a custom board: the reason and any WIP
/// override, automatic for a return.
fn custom_note(mv: &Move<'_>, over: bool, returned: bool) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(reason) = mv.reason.map(str::trim).filter(|r| !r.is_empty()) {
        parts.push(reason.to_string());
    }
    if over && returned {
        parts.push(guards::RETURNED_OVER_WIP.to_string());
    } else if over {
        let reason = mv.override_reason.unwrap_or_default().trim();
        parts.push(format!("WIP override: {reason}"));
    }
    (!parts.is_empty()).then(|| parts.join(" · "))
}

/// What stands between the item and each other column, in board order, for
/// the actor asking, read from one snapshot. No reason or override is
/// assumed, so a column that needs one lists it.
pub fn item_move_check(
    db: &Db,
    id: &str,
    actor: &Actor<'_>,
) -> anyhow::Result<Vec<(BoardColumn, Vec<Unmet>)>> {
    db.board_read(|t| {
        let (item, project_id, who) = load_in(t, id, actor)?;
        let ctx = t.move_context(&project_id, &item)?;
        let columns = board_columns(t, &item)?;
        let from = custom_from(t, &item, &columns)?.cloned();
        let draft = super::edit::not_live(&item);
        let mut out = Vec::new();
        for to in columns.iter().filter(|c| c.key != item.column_key) {
            if let Some(draft) = &draft {
                out.push((to.clone(), vec![draft.clone()]));
                continue;
            }
            let mv = Move {
                to,
                reason: None,
                override_reason: None,
            };
            let refused = match &from {
                Some(from) => flow::evaluate(&item, from, &to.policy, &mv, &who, &ctx),
                None => guards::evaluate(&item, &mv, &who, &ctx),
            };
            out.push((to.clone(), refused));
        }
        Ok(out)
    })
}

#[cfg(test)]
mod tests;
