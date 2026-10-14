//! Mixed versions on a peer link (0.19.0 K6). Each end says in its hello
//! which kanban contract it speaks, and a link speaks the older of the two:
//!
//! - 2 (0.19): many boards per project, board ids in frames, attachments.
//! - 1 (0.18, which sends none): one board per project and no board ids.
//!
//! A home serving a 0.18 computer gives it the project's default board as
//! "the board". A 0.19 computer whose home is on 0.18 keeps the single board
//! it is sent and refuses, before asking, what that home can't do.

use bus::contract::board as c;
use serde_json::Value;

use crate::app::AppState;
use crate::kanban::feed::BoardChange;

/// The kanban contract this version speaks on a peer link.
pub const KANBAN_CONTRACT: u64 = 2;

/// The contract a hello says its sender speaks; none is 0.18's.
pub(super) fn spoken(hello: &Value) -> u64 {
    hello
        .get("kanban")
        .and_then(Value::as_u64)
        .filter(|v| *v >= 1)
        .unwrap_or(1)
}

/// The link to this peer carries many boards and board ids.
pub fn many_boards(app: &AppState, peer_id: &str) -> bool {
    app.peers.kanban(peer_id) >= KANBAN_CONTRACT
}

/// The project's default board here: what a 0.18 computer calls its board.
pub(super) fn default_board(app: &AppState, project_id: &str) -> Option<String> {
    app.db
        .default_board(project_id)
        .ok()
        .flatten()
        .map(|b| b.id)
}

/// Keeps only board `id` of a snapshot; false when it has no such board.
pub(super) fn only_board(board: &mut c::BoardSnapshot, id: &str) -> bool {
    board.boards.retain(|b| b.id == id);
    board.columns.retain(|col| col.board_id == id);
    board.cards.retain(|card| card.board_id == id);
    !board.boards.is_empty()
}

/// A snapshot as a 0.18 computer reads it: the default board alone.
pub(super) fn single_board(board: &mut c::BoardSnapshot) {
    let default = board
        .boards
        .iter()
        .find(|b| b.is_default)
        .or(board.boards.first())
        .map(|b| b.id.clone());
    if let Some(id) = default {
        only_board(board, &id);
    }
    board.boards.clear();
}

/// A tool's columns and cards as a 0.18 computer reads them: those of the
/// default board.
pub(super) fn single_board_result(result: &mut Value, default: &str) {
    for key in ["columns", "cards"] {
        if let Some(list) = result.get_mut(key).and_then(Value::as_array_mut) {
            list.retain(|v| {
                v.get("board_id")
                    .or_else(|| v.get("boardId"))
                    .and_then(Value::as_str)
                    .is_none_or(|id| id.is_empty() || id == default)
            });
        }
    }
}

/// Whether a 0.18 computer hears of a change: one on another board than
/// the default is news about a board it can't see.
pub(super) fn on_single_board(change: &BoardChange, default: Option<&str>) -> bool {
    let board = change
        .board_id
        .as_deref()
        .or_else(|| change.card.as_ref().map(|card| card.board_id.as_str()))
        .filter(|b| !b.is_empty());
    match (board, default) {
        (Some(board), Some(default)) => board == default,
        _ => true,
    }
}

/// The tools a 0.19 bot has that a 0.18 home lacks.
const NEWER_TOOLS: [&str; 8] = [
    "board_create",
    "board_update",
    "column_add",
    "column_update",
    "column_remove",
    "automation_set",
    "item_attach",
    "item_detach",
];

/// A tool call for a home on 0.18: refused when that home lacks the tool,
/// otherwise the arguments without board ids, since it has one board.
pub fn single_board_args(home: &str, name: &str, args: &Value) -> anyhow::Result<Value> {
    anyhow::ensure!(
        !NEWER_TOOLS.contains(&name),
        "{home} holds this board and runs an older Gravity, with one board per project and \
         no attachments: {name} works once {home} is updated"
    );
    let mut args = args.clone();
    if let Some(fields) = args.as_object_mut() {
        fields.remove("board_id");
        fields.remove("board");
    }
    Ok(args)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn snapshot() -> c::BoardSnapshot {
        let board = |id: &str, is_default| c::Board {
            id: id.into(),
            is_default,
            ..Default::default()
        };
        let column = |board: &str, key: &str| c::BoardColumn {
            board_id: board.into(),
            key: key.into(),
            ..Default::default()
        };
        let card = |board: &str, id: &str| c::ItemCard {
            board_id: board.into(),
            id: id.into(),
            ..Default::default()
        };
        c::BoardSnapshot {
            boards: vec![board("main", true), board("rs", false)],
            columns: vec![column("main", "todo"), column("rs", "rs-todo")],
            cards: vec![card("main", "H-1"), card("rs", "RS-2")],
            ..Default::default()
        }
    }

    #[test]
    fn a_hello_without_a_contract_is_from_0_18() {
        assert_eq!(spoken(&json!({"type": "peer_hello"})), 1);
        assert_eq!(spoken(&json!({"kanban": 0})), 1);
        assert_eq!(spoken(&json!({"kanban": 2})), 2);
    }

    #[test]
    fn a_0_18_computer_is_served_the_default_board_alone() {
        let mut board = snapshot();
        single_board(&mut board);
        assert!(board.boards.is_empty());
        assert_eq!(board.columns.len(), 1);
        assert_eq!(board.columns[0].key, "todo");
        assert_eq!(board.cards.len(), 1);
        assert_eq!(board.cards[0].id, "H-1");

        let mut one = snapshot();
        assert!(only_board(&mut one, "rs"));
        assert_eq!(one.cards[0].id, "RS-2");
        assert!(!only_board(&mut snapshot(), "gone"));

        let mut result = json!({"columns": [{"board_id": "main"}, {"board_id": "rs"}],
                                "cards": [{"boardId": "rs"}, {"id": "H-1"}]});
        single_board_result(&mut result, "main");
        assert_eq!(
            result,
            json!({"columns": [{"board_id": "main"}], "cards": [{"id": "H-1"}]})
        );
    }

    #[test]
    fn a_home_on_0_18_is_asked_nothing_it_lacks() {
        let refused = single_board_args("mac", "column_add", &json!({})).unwrap_err();
        assert!(
            refused.to_string().contains("mac holds this board"),
            "{refused}"
        );
        let args = single_board_args(
            "mac",
            "item_create",
            &json!({"title": "t", "board_id": "b"}),
        )
        .unwrap();
        assert_eq!(args, json!({"title": "t"}));
    }
}
