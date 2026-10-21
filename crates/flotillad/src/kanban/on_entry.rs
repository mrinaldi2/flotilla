//! The board runs the SDLC (0.24.0, `board_flow_v1`): a card entering a
//! column with `on_entry` goes to a holder of that role, however it got
//! there (a bot's move, the owner's, a fact's, a new card, a finalized
//! draft). Run once the move has committed, so a failure here never undoes
//! the move: it is logged.
//!
//! - A card already assigned to a holder of the role keeps its assignee.
//! - Otherwise the card goes to the bot that last held it in that role, if
//!   it still holds it (a card sent back goes to its Developer again), else
//!   to the holder `by_role` picks: awake and free, then asleep.
//! - Nobody holds the role: the card keeps its assignee and waits, which
//!   Needs you shows as a `ROLE_UNHELD` row until a bot holds the role.
//! - A card sent back from testing (`sent_back`) gets the mover's reason
//!   as a comment.

use crate::actor::Actor;
use crate::app::AppState;
use crate::kanban::by_role;
use crate::kanban::feed::{card_after_commit, Change, ChangeKind, FeedWriter};
use crate::kanban::model::{BoardColumn, ColumnCategory, ColumnKind, Item, ItemState, Role};
use crate::kanban::notice_text::Notice;
use crate::kanban::notices;

mod text;

pub use text::role_name;

/// How the card entered its column.
#[derive(Clone, Copy)]
pub struct Entry<'a> {
    /// The column it left; `None` for a new card or a finalized draft.
    pub from: Option<&'a str>,
    /// The mover's reason, for a card sent back.
    pub reason: Option<&'a str>,
    /// Who moved it: a fact's move is the daemon's.
    pub by: &'a Actor<'a>,
}

/// What the column did with the card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The column assigns nobody, or the card isn't live.
    Nothing,
    /// The assignee already holds the role.
    Kept,
    Assigned(String),
    /// Nobody holds the role: the card waits.
    Unheld(Role),
}

/// Assign `item_id` as its column says, after its move committed; its push
/// goes out on `feed`, after the move's.
pub fn entered(
    app: &AppState,
    feed: &mut FeedWriter<'_>,
    item_id: &str,
    entry: Entry<'_>,
) -> Outcome {
    run(app, feed, item_id, entry).unwrap_or_else(|e| {
        tracing::warn!(item_id, error = %e, "on-entry assignment failed");
        Outcome::Nothing
    })
}

/// After a bot's or the owner's move, once its push is out: assign the card
/// as its new column says, then tell its assignee of the move, unless the
/// card went to another bot, whose old assignee is told that instead.
pub fn after_move(app: &AppState, feed: &mut FeedWriter<'_>, item: &Item, entry: Entry<'_>) {
    if !entered(app, feed, &item.id, entry).reassigned() {
        notices::tell_assignee(app, item, entry.by, Notice::Moved);
    }
}

impl Outcome {
    /// The card went to another bot: its old assignee's move notice is
    /// replaced by being told it isn't theirs any more.
    pub fn reassigned(&self) -> bool {
        matches!(self, Outcome::Assigned(_))
    }
}

/// [`entered`] for each card a fact moved, as `api::publish_moves` lists
/// them: the daemon moved them.
pub fn entered_all(app: &AppState, feed: &mut FeedWriter<'_>, moved: &[(String, String)]) {
    for (item_id, from) in moved {
        let entry = Entry {
            from: Some(from),
            reason: None,
            by: &Actor::Daemon,
        };
        entered(app, feed, item_id, entry);
    }
}

/// A bot holds `role` now: each card that waits for it goes to a holder,
/// as if it entered its column again.
pub fn role_held(app: &AppState, feed: &mut FeedWriter<'_>, project_id: &str, role: Role) {
    let waits = app.db.role_waits(project_id).unwrap_or_else(|e| {
        tracing::warn!(project_id, error = %e, "cards waiting for a role not read");
        Vec::new()
    });
    for wait in waits.into_iter().filter(|w| w.role == role) {
        let entry = Entry {
            from: None,
            reason: None,
            by: &Actor::Daemon,
        };
        entered(app, feed, &wait.item_id, entry);
    }
}

struct Read {
    item: Item,
    project_id: String,
    columns: Vec<BoardColumn>,
}

impl Read {
    fn column(&self, key: &str) -> Option<&BoardColumn> {
        self.columns.iter().find(|c| c.key == key)
    }
}

fn read(app: &AppState, item_id: &str) -> anyhow::Result<Option<Read>> {
    app.db.board_read(|t| {
        let (Some(item), Some(project_id)) = (t.item(item_id)?, t.item_project(item_id)?) else {
            return Ok(None);
        };
        let columns = t
            .columns(&project_id)?
            .into_iter()
            .filter(|c| item.board_id.is_empty() || c.board_id == item.board_id)
            .collect();
        Ok(Some(Read {
            item,
            project_id,
            columns,
        }))
    })
}

/// A move back from testing: to an earlier active column, from one that
/// assigns the Tester (or a release-gated Review or Verify).
pub fn sent_back(from: &BoardColumn, to: &BoardColumn) -> bool {
    let testing = from.on_entry == Some(Role::Tester)
        || matches!(
            from.category,
            ColumnCategory::Review | ColumnCategory::Verify
        );
    to.ord < from.ord && to.kind == ColumnKind::Active && testing
}

fn run(
    app: &AppState,
    feed: &mut FeedWriter<'_>,
    item_id: &str,
    entry: Entry<'_>,
) -> anyhow::Result<Outcome> {
    let Some(read) = read(app, item_id)? else {
        return Ok(Outcome::Nothing);
    };
    if read.item.state != ItemState::Live {
        return Ok(Outcome::Nothing);
    }
    let Some(to) = read.column(&read.item.column_key).cloned() else {
        return Ok(Outcome::Nothing);
    };
    let back = entry
        .from
        .and_then(|from| read.column(from))
        .is_some_and(|from| sent_back(from, &to));
    let reason = entry.reason.map(str::trim).filter(|r| !r.is_empty());
    let note = back
        .then_some(reason)
        .flatten()
        .map(|reason| format!("Sent back to {}: {reason}", to.name));
    let holder = match to.on_entry {
        Some(role) => Some((role, pick(app, &read, role)?)),
        None => None,
    };
    // One write: the reason, then the assignment or the wait, on the card
    // as it is now.
    let written = app.db.board_tx(|t| {
        if let Some(note) = &note {
            t.add_item_comment(item_id, note, None, entry.by)?;
        }
        t.clear_role_wait(item_id)?;
        let now = t.item(item_id)?;
        let same = now.as_ref().is_some_and(|i| {
            i.column_key == read.item.column_key && i.assignee == read.item.assignee
        });
        Ok(match (&holder, now) {
            (Some((role, Pick::Nobody)), Some(now)) if same => {
                t.set_role_wait(&read.project_id, &now, *role)?;
                None
            }
            (Some((role, Pick::Bot(bot))), Some(now)) if same => {
                match t.assign_on_entry(item_id, now.version, bot, *role)? {
                    crate::db::Write::Done(item) => Some(item),
                    crate::db::Write::Conflict(_) => None,
                }
            }
            _ => None,
        })
    })?;
    let (role, item) = match (holder, written) {
        (Some((role, Pick::Bot(_))), Some(item)) => (role, item),
        (Some((_, Pick::Kept)), _) => return Ok(touched(app, feed, &read, note, Outcome::Kept)),
        (Some((role, Pick::Nobody)), _) => {
            return Ok(touched(app, feed, &read, note, Outcome::Unheld(role)))
        }
        _ => return Ok(touched(app, feed, &read, note, Outcome::Nothing)),
    };
    let holder = item.assignee.clone().unwrap_or_default();
    feed.publish_by_daemon(Change {
        project_id: &read.project_id,
        kind: ChangeKind::ItemUpserted,
        item_id,
        card: card_after_commit(&app.db, item_id),
        from_column: None,
    });
    if let Some(previous) = read.item.assignee.as_deref() {
        notices::tell_off(app, previous, &item, entry.by);
    }
    let reason = back.then_some(reason).flatten();
    text::tell(app, &item, &to, &read.columns, role, entry, reason);
    Ok(Outcome::Assigned(holder))
}

/// Who a column's role gives the card to.
enum Pick {
    /// Its assignee holds the role already.
    Kept,
    Bot(String),
    Nobody,
}

/// The bot that last held the card, if it holds the role, so a card sent
/// back goes to its Developer again; else the readiest holder.
fn pick(app: &AppState, read: &Read, role: Role) -> anyhow::Result<Pick> {
    let roles = app.db.project_roles(&read.project_id)?;
    let holds = |bot: &str| roles.iter().any(|r| r.role == role && r.bot_id == bot);
    if read.item.assignee.as_deref().is_some_and(holds) {
        return Ok(Pick::Kept);
    }
    let before = app
        .db
        .board_read(|t| t.assignees_newest_first(&read.item.id))?;
    let holder = match before.into_iter().find(|bot| holds(bot)) {
        Some(bot) => Some(bot),
        None => by_role::holder(app, &read.project_id, role, &read.item.id)?,
    };
    Ok(holder.map_or(Pick::Nobody, Pick::Bot))
}

/// A card that kept its assignee: its reason, if one was posted, goes out
/// as a change to the card.
fn touched(
    app: &AppState,
    feed: &mut FeedWriter<'_>,
    read: &Read,
    note: Option<String>,
    outcome: Outcome,
) -> Outcome {
    if note.is_some() {
        feed.publish(Change {
            project_id: &read.project_id,
            kind: ChangeKind::ItemUpserted,
            item_id: &read.item.id,
            card: card_after_commit(&app.db, &read.item.id),
            from_column: None,
        });
    }
    outcome
}

#[cfg(test)]
mod tests;
