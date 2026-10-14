//! A card moves to Doing when its assignee starts work on it (0.19.0, card
//! flow): the `work.started` fact, which a custom board maps to a column in
//! its automations (the first board, Simple + Review, maps it to Doing).
//!
//! "Starts work" is the assignee's first `item_get` or `item_update` of its
//! card while the card sits in an open column. Once the assignee has moved
//! the card itself since it was assigned (back to To do, because it isn't
//! ready), its reads no longer start it: the bot moves it on itself when it
//! has its answers. A release-gated board never maps the fact, so its
//! guards decide every move as before.

use std::sync::Arc;

use crate::actor::Actor;
use crate::app::AppState;
use crate::db::BoardTx;
use crate::kanban::api::report_fact;
use crate::kanban::facts::Fact;
use crate::kanban::feed::{card_after_commit, Change, ChangeKind};
use crate::kanban::model::{BoardFlow, ColumnKind, ItemEventKind, ItemState};

/// Start `item_id` for `bot` if this touch starts it: the name of the
/// column it moved to, or `None`. A failure is logged and moves nothing;
/// the bot's own call stands.
pub fn on_touch(app: &Arc<AppState>, bot: &bus::Bot, item_id: &str) -> Option<String> {
    match start(app, bot, item_id) {
        Ok(moved) => moved,
        Err(e) => {
            tracing::warn!(bot_id = %bot.id, item_id, error = %e, "card not started");
            None
        }
    }
}

fn start(app: &Arc<AppState>, bot: &bus::Bot, item_id: &str) -> anyhow::Result<Option<String>> {
    let mut feed = app.board.writer();
    let moved = app.db.board_tx(|t| {
        let Some((project_id, column)) = startable(t, bot, item_id)? else {
            return Ok(None);
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
        Ok(from.map(|from| (project_id, from, column)))
    })?;
    let Some((project_id, from, column)) = moved else {
        return Ok(None);
    };
    crate::kanban::stuck::forget(&project_id, item_id);
    feed.publish(Change {
        project_id: &project_id,
        kind: ChangeKind::ItemMoved,
        item_id,
        card: card_after_commit(&app.db, item_id),
        from_column: Some(from),
    });
    Ok(Some(column))
}

/// The project and the name of the column the touch would move the card
/// to, when it starts it.
fn startable(
    t: &BoardTx<'_>,
    bot: &bus::Bot,
    item_id: &str,
) -> anyhow::Result<Option<(String, String)>> {
    let Some(item) = t.item(item_id)? else {
        return Ok(None);
    };
    let project_id = t.item_project(item_id)?.unwrap_or_default();
    if item.state != ItemState::Live
        || item.assignee.as_deref() != Some(bot.id.as_str())
        || project_id != bot.project_id
    {
        return Ok(None);
    }
    let Some(board) = t
        .board(&item.board_id)?
        .filter(|b| b.flow == BoardFlow::Custom)
    else {
        return Ok(None);
    };
    let Some(target) = board.automations.get(Fact::WorkStarted.name()) else {
        return Ok(None);
    };
    let columns = t.columns(&project_id)?;
    let here = columns.iter().find(|c| c.key == item.column_key);
    let to = columns.iter().find(|c| &c.key == target);
    let (Some(here), Some(to)) = (here, to) else {
        return Ok(None);
    };
    if here.kind != ColumnKind::Open || here.key == to.key || !armed(t, item_id, &bot.id)? {
        return Ok(None);
    }
    Ok(Some((project_id, to.name.clone())))
}

/// Whether the assignee hasn't moved the card itself since it was last
/// assigned to it.
fn armed(t: &BoardTx<'_>, item_id: &str, bot_id: &str) -> anyhow::Result<bool> {
    let (events, _) = t.item_history(item_id, None, 100_000)?;
    let actor = format!("bot:{bot_id}");
    let since = events
        .iter()
        .rposition(|e| e.kind == ItemEventKind::Assigned && e.to.as_deref() == Some(bot_id))
        .map_or(0, |at| at + 1);
    Ok(!events[since..]
        .iter()
        .any(|e| e.kind == ItemEventKind::Moved && e.actor == actor))
}
