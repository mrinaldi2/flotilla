//! Who made each board change, and why (0.19.0): read from the history the
//! change itself recorded, so every writer is attributed the same way, the
//! ones added later included, without passing an actor to the feed.
//!
//! The feed reads an item's history events newer than the last it read for
//! that item, starting from the newest event when the daemon started. Board
//! writers publish after they commit and hold the feed across both, so the
//! events read at publish time are the ones that change wrote.
//!
//! The connection a change was asked on travels the same way, per thread:
//! the WebSocket handler runs each board request inside [`on_connection`],
//! and a client skips the activity it asked for itself.

use std::cell::Cell;
use std::collections::HashMap;
use std::sync::Mutex;

use super::feed::ChangeKind;
use super::model::{ItemEvent, ItemEventKind};
use crate::db::Db;

/// The actor of a change the daemon made on its own: a fact (a PR merged, a
/// release shipped) or a system move.
pub const DAEMON: &str = "daemon";

/// The most history events one publish reads for one item.
const MAX_EVENTS: u32 = 50;

thread_local! {
    static ORIGIN: Cell<Option<u64>> = const { Cell::new(None) };
}

/// Run `work` as connection `conn`'s request: the board changes it publishes
/// name that connection as their origin.
pub fn on_connection<T>(conn: u64, work: impl FnOnce() -> T) -> T {
    struct Reset(Option<u64>);
    impl Drop for Reset {
        fn drop(&mut self) {
            ORIGIN.with(|o| o.set(self.0));
        }
    }
    let _reset = Reset(ORIGIN.with(|o| o.replace(Some(conn))));
    work()
}

/// The connection whose request is running on this thread, if any.
pub(super) fn origin() -> Option<u64> {
    ORIGIN.with(Cell::get)
}

/// The history the feed reads attributions from.
pub(super) struct History {
    db: Db,
    floor: i64,
    seen: Mutex<HashMap<String, i64>>,
}

impl History {
    pub(super) fn new(db: Db) -> Self {
        let floor = db
            .board_read(|t| t.last_item_event_id())
            .unwrap_or_else(|e| {
                tracing::warn!(error = %e, "can't read the board history's end");
                0
            });
        Self {
            db,
            floor,
            seen: Mutex::default(),
        }
    }

    /// The item's events recorded since the last call for it, oldest first.
    /// A failed read is logged and attributes nothing: the push still goes.
    pub(super) fn since_last(&self, item_id: &str) -> Vec<ItemEvent> {
        let mut seen = self.seen.lock().unwrap_or_else(|e| e.into_inner());
        let after = seen.get(item_id).copied().unwrap_or(self.floor);
        let events = self
            .db
            .board_read(|t| t.item_history(item_id, Some(after), MAX_EVENTS))
            .map(|(events, _)| events)
            .unwrap_or_else(|e| {
                tracing::warn!(item_id, error = %e, "can't read who changed the item");
                Vec::new()
            });
        if let Some(last) = events.last() {
            seen.insert(item_id.to_string(), last.id);
        }
        events
    }
}

/// The actor and note of a change, from the events it recorded: a move's
/// own event for a move, else the newest one.
pub(super) fn attribution(
    kind: ChangeKind,
    events: &[ItemEvent],
) -> (Option<String>, Option<String>) {
    let moved = events.iter().rev().find(|e| e.kind == ItemEventKind::Moved);
    match (kind, moved) {
        (ChangeKind::ItemMoved, Some(e)) => (Some(e.actor.clone()), e.note.clone()),
        _ => (events.last().map(|e| e.actor.clone()), None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(id: i64, kind: ItemEventKind, actor: &str, note: Option<&str>) -> ItemEvent {
        ItemEvent {
            id,
            item_id: "P-1".into(),
            at: chrono::Utc::now(),
            actor: actor.into(),
            kind,
            from: None,
            to: None,
            field: None,
            note: note.map(str::to_string),
        }
    }

    #[test]
    fn a_move_is_attributed_to_its_own_event() {
        let events = [
            event(1, ItemEventKind::Moved, "bot:b1", Some("started")),
            event(2, ItemEventKind::Edited, "user", None),
        ];
        assert_eq!(
            attribution(ChangeKind::ItemMoved, &events),
            (Some("bot:b1".into()), Some("started".into()))
        );
        assert_eq!(
            attribution(ChangeKind::ItemUpserted, &events),
            (Some("user".into()), None)
        );
        assert_eq!(attribution(ChangeKind::ItemMoved, &[]), (None, None));
    }

    #[test]
    fn the_origin_holds_for_the_request_and_no_longer() {
        assert_eq!(origin(), None);
        let inner = on_connection(7, || {
            let nested = on_connection(8, origin);
            (origin(), nested)
        });
        assert_eq!(inner, (Some(7), Some(8)));
        assert_eq!(origin(), None);
    }
}
