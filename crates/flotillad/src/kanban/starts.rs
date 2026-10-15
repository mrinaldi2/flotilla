//! A card moves to Doing when its assignee starts work on it (0.19.0, card
//! flow): the `work.started` fact, which a custom board maps to a column in
//! its automations (the first board, Simple + Review, maps it to Doing).
//!
//! "Starts work" is the assignee's first `item_get` or `item_update` of its
//! card while the card sits in an open column. The move is the assignee's
//! own, made for it: it happens only when the assignee could make it with
//! `item_move` (the column's movers, rules and WIP limit), and otherwise
//! the reply says what stops it. Once anyone has moved the card since it
//! was assigned (the bot back to To do because it isn't ready, the owner
//! because it isn't wanted yet), reads no longer start it: whoever moves it
//! on does so with `item_move`. A release-gated board never maps the fact,
//! so its guards decide every move as before.

use std::sync::Arc;

use crate::actor::Actor;
use crate::app::AppState;
use crate::db::BoardTx;
use crate::kanban::api::report_fact;
use crate::kanban::facts::Fact;
use crate::kanban::feed::{card_after_commit, Change, ChangeKind};
use crate::kanban::model::{BoardColumn, BoardFlow, ColumnKind, ItemEventKind, ItemState};
use crate::kanban::moves::{load_in, unmet_for};

/// What a touch did to the card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Touch {
    /// Nothing: not a start.
    None,
    /// It moved to this column.
    Started(String),
    /// It would have started, but the move isn't one the assignee may make
    /// now: the column's name and what stops it.
    Held(String, Vec<String>),
}

impl Touch {
    /// The reply's key and text for it, if any.
    pub fn reply(&self) -> Option<(&'static str, String)> {
        match self {
            Touch::None => None,
            Touch::Started(column) => Some(("started", started_note(column))),
            Touch::Held(column, why) => Some((
                "not_started",
                format!(
                    "It stays where it is: it can't move to {column} yet. {} Move it with \
                     item_move when it can go, or ask the lead.",
                    why.join(" ")
                ),
            )),
        }
    }
}

/// What the reply says when the call started the bot's work on its card.
pub fn started_note(column: &str) -> String {
    format!(
        "You started work on this card, so it moved to {column}. When the work is done, \
         comment the result, link or attach what you made and move it on with item_move. If \
         it isn't ready or something is unclear, move it back and ask the owner \
         (message_owner with asks: true and item)."
    )
}

/// Start `item_id` for `bot` if this touch starts it. A failure is logged
/// and moves nothing; the bot's own call stands.
pub fn on_touch(app: &Arc<AppState>, bot: &bus::Bot, item_id: &str) -> Touch {
    app.keeper.read(&bot.id, item_id, chrono::Utc::now());
    match start(app, bot, item_id) {
        Ok(touch) => touch,
        Err(e) => {
            tracing::warn!(bot_id = %bot.id, item_id, error = %e, "card not started");
            Touch::None
        }
    }
}

fn start(app: &Arc<AppState>, bot: &bus::Bot, item_id: &str) -> anyhow::Result<Touch> {
    let mut feed = app.board.writer();
    let moved = app.db.board_tx(|t| {
        let (project_id, column) = match startable(t, bot, item_id)? {
            Start::No => return Ok(Err(Touch::None)),
            Start::Held(name, why) => return Ok(Err(Touch::Held(name, why))),
            Start::Yes(project_id, name) => (project_id, name),
        };
        let note = format!(
            "{}: {} started work on it",
            Fact::WorkStarted.name(),
            bot.name
        );
        let from = report_fact(
            t,
            &project_id,
            item_id,
            Fact::WorkStarted,
            &note,
            false,
            &Actor::Daemon,
        )?;
        Ok(from
            .map(|from| (project_id, from, column))
            .ok_or(Touch::None))
    })?;
    let (project_id, from, column) = match moved {
        Ok(moved) => moved,
        Err(touch) => return Ok(touch),
    };
    crate::kanban::stuck::forget(&project_id, item_id);
    feed.publish(Change {
        project_id: &project_id,
        kind: ChangeKind::ItemMoved,
        item_id,
        card: card_after_commit(&app.db, item_id),
        from_column: Some(from),
    });
    Ok(Touch::Started(column))
}

/// Whether the touch starts the card.
enum Start {
    No,
    /// The project and the name of the column it moves to.
    Yes(String, String),
    /// The column's name, and why the assignee may not move it there now.
    Held(String, Vec<String>),
}

fn startable(t: &BoardTx<'_>, bot: &bus::Bot, item_id: &str) -> anyhow::Result<Start> {
    let Some(item) = t.item(item_id)? else {
        return Ok(Start::No);
    };
    let project_id = t.item_project(item_id)?.unwrap_or_default();
    if item.state != ItemState::Live
        || item.assignee.as_deref() != Some(bot.id.as_str())
        || project_id != bot.project_id
    {
        return Ok(Start::No);
    }
    let Some(board) = t
        .board(&item.board_id)?
        .filter(|b| b.flow == BoardFlow::Custom)
    else {
        return Ok(Start::No);
    };
    let Some(target) = board.automations.get(Fact::WorkStarted.name()) else {
        return Ok(Start::No);
    };
    let columns: Vec<BoardColumn> = t
        .columns(&project_id)?
        .into_iter()
        .filter(|c| item.board_id.is_empty() || c.board_id == item.board_id)
        .collect();
    let here = columns.iter().find(|c| c.key == item.column_key);
    let to = columns.iter().find(|c| &c.key == target);
    let (Some(here), Some(to)) = (here, to) else {
        return Ok(Start::No);
    };
    if here.kind != ColumnKind::Open || here.key == to.key || !armed(t, item_id, &bot.id)? {
        return Ok(Start::No);
    }
    // The assignee's own move, made for it: only one it may make itself.
    let actor = Actor::Bot {
        id: &bot.id,
        project_id: &bot.project_id,
    };
    let (_, _, who) = load_in(t, item_id, &actor)?;
    let unmet = unmet_for(t, &project_id, &item, &columns, to, &who)?;
    if !unmet.is_empty() {
        let why = unmet.into_iter().map(|u| u.text).collect();
        return Ok(Start::Held(to.name.clone(), why));
    }
    Ok(Start::Yes(project_id, to.name.clone()))
}

/// Whether nobody has moved the card since it was last assigned to the
/// bot: a move by the bot (it isn't ready), the owner (not now) or anyone
/// else says where the card belongs, and a read doesn't undo it.
fn armed(t: &BoardTx<'_>, item_id: &str, bot_id: &str) -> anyhow::Result<bool> {
    let (events, _) = t.item_history(item_id, None, 100_000)?;
    let since = events
        .iter()
        .rposition(|e| e.kind == ItemEventKind::Assigned && e.to.as_deref() == Some(bot_id))
        .map_or(0, |at| at + 1);
    Ok(!events[since..]
        .iter()
        .any(|e| e.kind == ItemEventKind::Moved))
}
