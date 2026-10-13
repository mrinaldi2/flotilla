//! The port other modules use to change the board (0.19.0 K1). Releases,
//! PRs and meetings change cards only through these functions; Kanban
//! never calls into them.

use std::sync::Arc;

use crate::actor::Actor;
use crate::app::AppState;
use crate::db::{BoardTx, MoveTo, NewItem, Write};
use crate::decisions::{conflict, not_found};
use crate::kanban::feed::{card_after_commit, Change, ChangeKind, FeedWriter};
use crate::kanban::guards::{self, Move, Who};
use crate::kanban::model::{ColumnCategory, Item, ItemComment, ItemLink, LinkKind};

/// A move another module makes on the system's authority (a release ruling,
/// a PR event): the guards' daemon path, recorded under `actor` (the owner
/// who ruled, or the tester who confirmed). Returns the column it left, or
/// `None` when it was already there.
pub fn system_move(
    t: &BoardTx<'_>,
    project_id: &str,
    item_id: &str,
    to: ColumnCategory,
    note: &str,
    first: bool,
    actor: &Actor<'_>,
) -> anyhow::Result<Option<String>> {
    let item = t
        .item(item_id)?
        .ok_or_else(|| not_found(format!("no item {item_id}")))?;
    let columns = t.columns(project_id)?;
    let column = columns
        .iter()
        .find(|c| c.category == to)
        .ok_or_else(|| anyhow::anyhow!("this board has no {} column", to.as_str()))?;
    if column.key == item.column_key {
        return Ok(None);
    }
    let ctx = t.move_context(project_id, &item)?;
    let mv = Move {
        to: column,
        reason: Some(note),
        override_reason: None,
    };
    let unmet = guards::evaluate(&item, &mv, &Who::Daemon, &ctx);
    anyhow::ensure!(
        unmet.is_empty(),
        "the daemon can't move {item_id}: {unmet:?}"
    );
    // Returned work over a WIP limit is flagged as F1 asks; nothing else is.
    let over = first && guards::over_limit(&item, column, &ctx).is_some();
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
    match t.move_item(item_id, item.version, &to, actor)? {
        Write::Done(_) => Ok(Some(item.column_key)),
        Write::Conflict(_) => Err(conflict(format!("{item_id} changed while moving it"))),
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
