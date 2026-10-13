//! The port other modules use to change the board (0.19.0 K1). Releases,
//! PRs and meetings change cards only through these functions; Kanban
//! never calls into them.

use std::sync::Arc;

use crate::actor::Actor;
use crate::app::AppState;
use crate::db::{BoardTx, MoveTo, NewItem, Write};
use crate::decisions::{conflict, not_found};
use crate::kanban::facts::Fact;
use crate::kanban::feed::{card_after_commit, Change, ChangeKind, FeedWriter};
use crate::kanban::guards::{self, Move, Who};
use crate::kanban::model::{BoardColumn, BoardFlow, Item, ItemComment, ItemLink, LinkKind};

/// Report a fact about an item and let its board decide where it goes
/// (0.19.0 K3). A release-gated board moves it the way 0.18 did; a custom
/// board moves it to the column its automations map the fact to, and keeps
/// the fact on the item when none is mapped. The move is recorded under
/// `actor` (the owner who ruled, the tester who confirmed). Returns the
/// column it left, or `None` when it didn't move.
pub fn report_fact(
    t: &BoardTx<'_>,
    project_id: &str,
    item_id: &str,
    fact: Fact,
    note: &str,
    first: bool,
    actor: &Actor<'_>,
) -> anyhow::Result<Option<String>> {
    let item = t
        .item(item_id)?
        .ok_or_else(|| not_found(format!("no item {item_id}")))?;
    let board = t.board(&item.board_id)?;
    let columns: Vec<BoardColumn> = t
        .columns(project_id)?
        .into_iter()
        .filter(|c| item.board_id.is_empty() || c.board_id == item.board_id)
        .collect();
    let column = match board.filter(|b| b.flow == BoardFlow::Custom) {
        Some(board) => match board.automations.get(fact.name()) {
            Some(key) => columns.iter().find(|c| &c.key == key).ok_or_else(|| {
                anyhow::anyhow!(
                    "{} maps {} to a missing column {key}",
                    board.name,
                    fact.name()
                )
            })?,
            None => {
                t.record_fact(item_id, fact.name(), Some(note), actor)?;
                return Ok(None);
            }
        },
        None => {
            let to = fact.release_gated();
            columns
                .iter()
                .find(|c| c.category == to)
                .ok_or_else(|| anyhow::anyhow!("this board has no {} column", to.as_str()))?
        }
    };
    system_move(t, project_id, &item, column, note, first, actor)
}

/// The daemon's own move of `item` to `column`: the guards' daemon path.
fn system_move(
    t: &BoardTx<'_>,
    project_id: &str,
    item: &Item,
    column: &BoardColumn,
    note: &str,
    first: bool,
    actor: &Actor<'_>,
) -> anyhow::Result<Option<String>> {
    if column.key == item.column_key {
        return Ok(None);
    }
    let ctx = t.move_context(project_id, item)?;
    let mv = Move {
        to: column,
        reason: Some(note),
        override_reason: None,
    };
    let unmet = guards::evaluate(item, &mv, &Who::Daemon, &ctx);
    anyhow::ensure!(
        unmet.is_empty(),
        "the daemon can't move {}: {unmet:?}",
        item.id
    );
    // Returned work over a WIP limit is flagged as F1 asks; nothing else is.
    let over = first && guards::over_limit(item, column, &ctx).is_some();
    let note = if over {
        format!("{note} · {}", guards::RETURNED_OVER_WIP)
    } else {
        note.to_string()
    };
    let to = MoveTo {
        column: &column.key,
        note: Some(&note),
        wip_override: over,
        first,
    };
    match t.move_item(&item.id, item.version, &to, actor)? {
        Write::Done(_) => Ok(Some(item.column_key.clone())),
        Write::Conflict(_) => Err(conflict(format!("{} changed while moving it", item.id))),
    }
}

/// Push each system move, once its transaction has committed.
pub fn publish_moves(
    app: &Arc<AppState>,
    feed: &mut FeedWriter<'_>,
    project_id: &str,
    moved: &[(String, String)],
) {
    for (item_id, from) in moved {
        feed.publish(Change {
            project_id,
            kind: ChangeKind::ItemMoved,
            item_id,
            card: card_after_commit(&app.db, item_id),
            from_column: Some(from.clone()),
        });
    }
}

/// Push the cards another module changed without moving them (a release's
/// package changed, so their version did), once the transaction has
/// committed: otherwise a mirror keeps the old version and its next write
/// conflicts.
pub fn publish_touched(
    app: &Arc<AppState>,
    feed: &mut FeedWriter<'_>,
    project_id: &str,
    touched: &[String],
    moved: &[(String, String)],
) {
    for item_id in touched {
        if moved.iter().any(|(id, _)| id == item_id) {
            continue;
        }
        feed.publish(Change {
            project_id,
            kind: ChangeKind::ItemUpserted,
            item_id,
            card: card_after_commit(&app.db, item_id),
            from_column: None,
        });
    }
}

/// Link an item to something another module owns: a PR, a branch, a
/// meeting, a task.
pub fn link(
    t: &BoardTx<'_>,
    item_id: &str,
    kind: LinkKind,
    target: &str,
    label: Option<&str>,
    actor: &Actor<'_>,
) -> anyhow::Result<ItemLink> {
    t.add_item_link(item_id, kind, target, label, actor)
}

/// Comment on an item on another module's behalf.
pub fn comment(
    t: &BoardTx<'_>,
    item_id: &str,
    body: &str,
    reply_to: Option<&str>,
    actor: &Actor<'_>,
) -> anyhow::Result<ItemComment> {
    t.add_item_comment(item_id, body, reply_to, actor)
}

/// Create an item for another module: a PR follow-up, a promoted meeting
/// action.
pub fn create(t: &BoardTx<'_>, new: &NewItem<'_>, actor: &Actor<'_>) -> anyhow::Result<Item> {
    t.create_item(new, actor)
}
