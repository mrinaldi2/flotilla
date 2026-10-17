//! Board pushes: every committed board change is published
//! here with the next number of its project's sequence, and the WS adapter
//! forwards it to the connections watching that project.
//!
//! A writer holds the feed (`writer()`) across its transaction and its
//! publish, so changes go out in the order they committed and a sequence
//! number always follows the change it names. A client reads `seq()` before
//! its snapshot: a change that lands in between is in the snapshot and
//! arrives again as a push, which is harmless because a push carries the
//! whole card.
//!
//! The sequence lives in memory and starts again at 0 when the daemon does.
//! Clients treat any number other than the last one plus one, lower ones
//! included, as missed pushes and refetch the board.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use tokio::sync::broadcast;

use super::feed_by::{self, History};
use super::model::{ItemCard, ItemEvent};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    ItemUpserted,
    ItemMoved,
    ItemRemoved,
    ColumnsChanged,
    SettingsChanged,
    /// A draft was created, changed, finalized or discarded (0.19.0 K4b):
    /// only the owner's apps hear of it, never a linked computer.
    DraftChanged,
    /// A plan node changed or appeared (0.21.0): its node is on the change.
    /// It takes no number of the project's sequence, since its push carries
    /// none, and stays on this computer like a draft.
    PlanChanged,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BoardChange {
    pub project_id: String,
    pub seq: u64,
    pub kind: ChangeKind,
    pub item_id: String,
    pub card: Option<ItemCard>,
    pub from_column: Option<String>,
    /// The board a structure change was to (0.19.0 K6), so a linked
    /// computer refetches that board alone. An item's is on its card.
    pub board_id: Option<String>,
    /// Who made it, as history stores actors (`user`, `device:<id>`,
    /// `bot:<id>`), or [`feed_by::DAEMON`]; unknown when nothing says.
    pub actor: Option<String>,
    /// A move's note: its reason, or the fact that moved it.
    pub note: Option<String>,
    /// The history events the change recorded, oldest first, for the
    /// owner's activity notices (`kanban::activity`).
    pub events: Vec<ItemEvent>,
    /// The WebSocket connection that asked for it, if one did.
    pub origin: Option<u64>,
    /// The node of a `PlanChanged`, as it is after the change.
    pub plan: Option<Box<crate::plan::model::Node>>,
}

/// How many changes a slow connection may fall behind before it is told to
/// resync instead.
const BACKLOG: usize = 1024;

/// The card to push for a change that has committed. A failed read must not
/// lose the push (ARCH-R8 F2): the change is published without its card, and
/// clients refetch that item.
pub fn card_after_commit(db: &crate::db::Db, item_id: &str) -> Option<ItemCard> {
    db.board_read(|t| t.card(item_id)).unwrap_or_else(|e| {
        tracing::warn!(item_id, error = %e, "card read after commit failed; pushing without it");
        None
    })
}

#[derive(Clone)]
pub struct BoardFeed {
    seqs: Arc<Mutex<HashMap<String, u64>>>,
    tx: broadcast::Sender<Arc<BoardChange>>,
    /// Where it reads who made each item change; none in unit tests.
    history: Option<Arc<History>>,
}

impl Default for BoardFeed {
    fn default() -> Self {
        Self::with_backlog(BACKLOG)
    }
}

impl BoardFeed {
    /// A feed that lets a connection fall `backlog` changes behind; tests
    /// use a small one to reach the resync path.
    pub fn with_backlog(backlog: usize) -> Self {
        Self {
            seqs: Arc::default(),
            tx: broadcast::channel(backlog).0,
            history: None,
        }
    }

    /// The feed attributing each item change from `db`'s history.
    pub fn with_history(mut self, db: crate::db::Db) -> Self {
        self.history = Some(Arc::new(History::new(db)));
        self
    }
}

impl BoardFeed {
    pub fn subscribe(&self) -> broadcast::Receiver<Arc<BoardChange>> {
        self.tx.subscribe()
    }

    /// The project's last published sequence number, 0 before any change.
    pub fn seq(&self, project_id: &str) -> u64 {
        self.lock().get(project_id).copied().unwrap_or(0)
    }

    /// Hold the feed for one board write and its publish. Every board
    /// writer takes this before its transaction, the MCP tools included.
    pub fn writer(&self) -> FeedWriter<'_> {
        FeedWriter {
            seqs: self.lock(),
            tx: &self.tx,
            history: self.history.as_deref(),
        }
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<String, u64>> {
        self.seqs.lock().unwrap_or_else(|e| e.into_inner())
    }
}

pub struct FeedWriter<'a> {
    seqs: MutexGuard<'a, HashMap<String, u64>>,
    tx: &'a broadcast::Sender<Arc<BoardChange>>,
    history: Option<&'a History>,
}

/// One change to publish; `seq` is assigned on publishing.
pub struct Change<'a> {
    pub project_id: &'a str,
    pub kind: ChangeKind,
    pub item_id: &'a str,
    pub card: Option<ItemCard>,
    pub from_column: Option<String>,
}

impl FeedWriter<'_> {
    /// Number the change and send it to every watcher; returns its number.
    /// A change to a draft's card goes out as `DraftChanged`, whatever the
    /// writer called it, so no board lists a draft.
    pub fn publish(&mut self, change: Change<'_>) -> u64 {
        self.publish_on(None, change)
    }

    /// [`Self::publish`], naming the board a structure change was to.
    pub fn publish_on(&mut self, board_id: Option<&str>, change: Change<'_>) -> u64 {
        self.publish_as(board_id, (None, None), change)
    }

    /// [`Self::publish`] for a change the daemon made on its own (a fact's
    /// move): its history names whoever reported the fact, but the change
    /// is the daemon's.
    pub fn publish_by_daemon(&mut self, change: Change<'_>) -> u64 {
        self.publish_as(None, (Some(feed_by::DAEMON.to_string()), None), change)
    }

    /// [`Self::publish`] for a change a board's home relayed: its actor and
    /// note are the home's, as it sent them.
    pub fn publish_relayed(
        &mut self,
        actor: Option<String>,
        note: Option<String>,
        change: Change<'_>,
    ) -> u64 {
        self.publish_as(None, (actor, note), change)
    }

    /// `given` is an actor and note that win over the history's.
    fn publish_as(
        &mut self,
        board_id: Option<&str>,
        given: (Option<String>, Option<String>),
        change: Change<'_>,
    ) -> u64 {
        let item_change = matches!(
            change.kind,
            ChangeKind::ItemUpserted | ChangeKind::ItemMoved | ChangeKind::DraftChanged
        );
        let events = match self.history {
            Some(history) if item_change && !change.item_id.is_empty() => {
                history.since_last(change.item_id)
            }
            _ => Vec::new(),
        };
        let (recorded, noted) = feed_by::attribution(change.kind, &events);
        let actor = given.0.or(recorded);
        let note = given.1.or(noted);
        let kind = match change.kind {
            ChangeKind::ItemUpserted | ChangeKind::ItemMoved
                if change.card.as_ref().is_some_and(|card| card.draft) =>
            {
                ChangeKind::DraftChanged
            }
            kind => kind,
        };
        let seq = self.seqs.entry(change.project_id.to_string()).or_insert(0);
        *seq += 1;
        // No receiver is no watcher, which is fine.
        let _ = self.tx.send(Arc::new(BoardChange {
            project_id: change.project_id.to_string(),
            seq: *seq,
            kind,
            item_id: change.item_id.to_string(),
            card: change.card,
            from_column: change.from_column,
            board_id: board_id.map(str::to_string),
            actor,
            note,
            events,
            origin: feed_by::origin(),
            plan: None,
        }));
        *seq
    }

    /// A plan node as it is after a change: sent to the project's watchers
    /// at the project's current number, which it doesn't take.
    pub fn publish_plan(&mut self, node: crate::plan::model::Node, actor: Option<String>) {
        let project_id = node.project_id.clone();
        let seq = self.seqs.get(&project_id).copied().unwrap_or(0);
        let _ = self.tx.send(Arc::new(BoardChange {
            project_id,
            seq,
            kind: ChangeKind::PlanChanged,
            item_id: String::new(),
            card: None,
            from_column: None,
            board_id: node.board_id.clone(),
            actor,
            note: None,
            events: Vec::new(),
            origin: feed_by::origin(),
            plan: Some(Box::new(node)),
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn change<'a>(project_id: &'a str, item_id: &'a str) -> Change<'a> {
        Change {
            project_id,
            kind: ChangeKind::ItemUpserted,
            item_id,
            card: None,
            from_column: None,
        }
    }

    #[test]
    fn each_project_counts_its_own_changes_from_one() {
        let feed = BoardFeed::default();
        let mut rx = feed.subscribe();
        assert_eq!(feed.seq("p1"), 0);
        let mut w = feed.writer();
        assert_eq!(w.publish(change("p1", "RB-001")), 1);
        assert_eq!(w.publish(change("p2", "X-001")), 1);
        assert_eq!(w.publish(change("p1", "RB-002")), 2);
        drop(w);
        assert_eq!((feed.seq("p1"), feed.seq("p2")), (2, 1));
        let got: Vec<(String, u64)> = (0..3)
            .map(|_| {
                let c = rx.try_recv().expect("published");
                (c.item_id.clone(), c.seq)
            })
            .collect();
        assert_eq!(
            got,
            [
                ("RB-001".into(), 1),
                ("X-001".into(), 1),
                ("RB-002".into(), 2)
            ]
        );
    }

    #[test]
    fn a_reader_waits_for_a_writer_to_publish() {
        let feed = BoardFeed::default();
        let mut w = feed.writer();
        let reader = {
            let feed = feed.clone();
            std::thread::spawn(move || feed.seq("p"))
        };
        std::thread::sleep(std::time::Duration::from_millis(20));
        w.publish(change("p", "RB-001"));
        drop(w);
        assert_eq!(reader.join().expect("reader"), 1);
    }
}
