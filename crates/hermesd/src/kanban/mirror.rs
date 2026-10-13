//! Boards whose home is a peer (B9, H-020 §1.3). A linked project's board
//! lives on one daemon; the others keep its last snapshot here, in their own
//! ids, kept current by the home's `board_event` relay. They serve reads from
//! it, so it is also the read-only board while the home is unreachable. It
//! lives in memory: a restart refetches it when the home's link comes up.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use bus::contract::board as c;

/// A peer's board as this daemon last saw it.
#[derive(Debug, Clone)]
pub struct Mirrored {
    /// The peer holding the board.
    pub peer_id: String,
    /// In this daemon's ids. Its `seq` is meaningless here: callers stamp
    /// the local feed's.
    pub snapshot: c::BoardSnapshot,
}

/// Local project id → the board mirrored for it.
#[derive(Clone, Default)]
pub struct BoardMirror {
    boards: Arc<Mutex<HashMap<String, Mirrored>>>,
}

impl BoardMirror {
    fn lock(&self) -> MutexGuard<'_, HashMap<String, Mirrored>> {
        self.boards.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn get(&self, project_id: &str) -> Option<Mirrored> {
        self.lock().get(project_id).cloned()
    }

    /// The peer holding the project's board, when one is known.
    pub fn home_peer(&self, project_id: &str) -> Option<String> {
        self.lock().get(project_id).map(|m| m.peer_id.clone())
    }

    pub fn set(&self, project_id: &str, peer_id: &str, snapshot: c::BoardSnapshot) {
        self.lock().insert(
            project_id.to_string(),
            Mirrored {
                peer_id: peer_id.to_string(),
                snapshot,
            },
        );
    }

    /// Replaces one board of a mirrored project with `part`, that board as
    /// its home sent it (0.19.0 K6): its columns and cards, with the
    /// project's settings and roles. A board `part` lacks is gone there.
    /// False when the project isn't mirrored from `peer_id`.
    pub fn set_board(
        &self,
        project_id: &str,
        peer_id: &str,
        board_id: &str,
        part: c::BoardSnapshot,
    ) -> bool {
        let mut boards = self.lock();
        let Some(held) = boards.get_mut(project_id).filter(|m| m.peer_id == peer_id) else {
            return false;
        };
        let whole = &mut held.snapshot;
        let default = part.boards.iter().any(|b| b.is_default);
        whole.boards.retain(|b| b.id != board_id);
        if default {
            whole.boards.iter_mut().for_each(|b| b.is_default = false);
        }
        whole.boards.extend(part.boards);
        whole.boards.sort_by_key(|b| (!b.is_default, b.ord));
        whole.columns.retain(|col| col.board_id != board_id);
        whole.columns.extend(part.columns);
        let order: Vec<String> = whole.boards.iter().map(|b| b.id.clone()).collect();
        let at = |id: &str| order.iter().position(|b| b == id).unwrap_or(order.len());
        whole
            .columns
            .sort_by_key(|col| (at(&col.board_id), col.ord));
        whole.cards.retain(|card| card.board_id != board_id);
        whole.cards.extend(part.cards);
        if part.settings.is_some() {
            whole.settings = part.settings;
            whole.roles = part.roles;
        }
        true
    }

    /// Forgets the board, when `peer_id` says it no longer holds it.
    pub fn forget(&self, project_id: &str, peer_id: &str) {
        let mut boards = self.lock();
        if boards.get(project_id).is_some_and(|m| m.peer_id == peer_id) {
            boards.remove(project_id);
        }
    }

    /// The mirrored project an item belongs to.
    pub fn project_of(&self, item_id: &str) -> Option<String> {
        self.lock()
            .iter()
            .find(|(_, m)| m.snapshot.cards.iter().any(|card| card.id == item_id))
            .map(|(project, _)| project.clone())
    }

    /// Takes a pushed card unless the mirror already holds a newer version
    /// (a refetch can overtake the pushes behind it). False when nothing
    /// changed, or the project isn't mirrored.
    pub fn apply_card(&self, project_id: &str, card: c::ItemCard) -> bool {
        let mut boards = self.lock();
        let Some(board) = boards.get_mut(project_id) else {
            return false;
        };
        let cards = &mut board.snapshot.cards;
        match cards.iter_mut().find(|held| held.id == card.id) {
            Some(held) if held.version > card.version => false,
            Some(held) => {
                *held = card;
                true
            }
            None => {
                cards.push(card);
                true
            }
        }
    }

    /// Drops a removed item. False when the project isn't mirrored.
    pub fn remove_card(&self, project_id: &str, item_id: &str) -> bool {
        let mut boards = self.lock();
        let Some(board) = boards.get_mut(project_id) else {
            return false;
        };
        board.snapshot.cards.retain(|card| card.id != item_id);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(id: &str, version: u64, column: &str) -> c::ItemCard {
        c::ItemCard {
            id: id.into(),
            version,
            column_key: column.into(),
            ..Default::default()
        }
    }

    #[test]
    fn a_pushed_card_never_rolls_back_a_newer_one() {
        let mirror = BoardMirror::default();
        assert!(!mirror.apply_card("p", card("H-1", 1, "ready")));
        mirror.set("p", "peer", c::BoardSnapshot::default());
        assert!(mirror.apply_card("p", card("H-1", 3, "doing")));
        assert!(!mirror.apply_card("p", card("H-1", 2, "ready")));
        assert!(mirror.apply_card("p", card("H-1", 4, "review")));
        let held = mirror.get("p").expect("mirrored").snapshot.cards;
        assert_eq!(held, [card("H-1", 4, "review")]);
        assert_eq!(mirror.project_of("H-1").as_deref(), Some("p"));
        assert!(mirror.remove_card("p", "H-1"));
        assert_eq!(mirror.project_of("H-1"), None);
    }

    #[test]
    fn one_board_is_replaced_and_the_others_kept() {
        let board = |id: &str, ord, is_default| c::Board {
            id: id.into(),
            ord,
            is_default,
            ..Default::default()
        };
        let column = |board: &str, key: &str, ord| c::BoardColumn {
            board_id: board.into(),
            key: key.into(),
            ord,
            ..Default::default()
        };
        let on = |board: &str, id: &str| c::ItemCard {
            board_id: board.into(),
            ..card(id, 1, "todo")
        };
        let mirror = BoardMirror::default();
        assert!(!mirror.set_board("p", "mac", "rs", c::BoardSnapshot::default()));
        mirror.set(
            "p",
            "mac",
            c::BoardSnapshot {
                boards: vec![board("main", 0, true), board("rs", 1, false)],
                columns: vec![column("main", "todo", 0), column("rs", "rs-todo", 0)],
                cards: vec![on("main", "H-1"), on("rs", "RS-2")],
                ..Default::default()
            },
        );
        assert!(!mirror.set_board("p", "imac", "rs", c::BoardSnapshot::default()));
        let part = c::BoardSnapshot {
            boards: vec![board("rs", 1, false)],
            columns: vec![column("rs", "rs-done", 1), column("rs", "rs-todo", 0)],
            cards: vec![on("rs", "RS-3")],
            ..Default::default()
        };
        assert!(mirror.set_board("p", "mac", "rs", part));
        let held = mirror.get("p").expect("mirrored").snapshot;
        let keys: Vec<&str> = held.columns.iter().map(|c| c.key.as_str()).collect();
        assert_eq!(keys, ["todo", "rs-todo", "rs-done"]);
        let cards: Vec<&str> = held.cards.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(cards, ["H-1", "RS-3"]);
        // A board its home no longer has leaves the mirror.
        assert!(mirror.set_board("p", "mac", "rs", c::BoardSnapshot::default()));
        let held = mirror.get("p").expect("mirrored").snapshot;
        assert_eq!(held.boards.len(), 1);
        assert_eq!(held.columns.len(), 1);
    }

    #[test]
    fn only_the_holding_peer_can_withdraw_a_board() {
        let mirror = BoardMirror::default();
        mirror.set("p", "mac", c::BoardSnapshot::default());
        mirror.forget("p", "imac");
        assert_eq!(mirror.home_peer("p").as_deref(), Some("mac"));
        mirror.forget("p", "mac");
        assert_eq!(mirror.home_peer("p"), None);
    }
}
