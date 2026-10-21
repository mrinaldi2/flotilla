//! The 0.24 default chain reaching boards made before it.

use super::board_tests::board;
use super::Db;
use crate::kanban::flow::Mover;
use crate::kanban::model::{BoardColumn, Role};
use crate::kanban::presets::Preset;

/// Back to boards from before 0.24: no column assigns a role, Done and
/// Ready keep their old movers, and nothing was added yet.
fn as_023(db: &Db) {
    let lead = serde_json::to_string(&[Mover::Lead]).unwrap();
    db.lock()
        .execute_batch(&format!(
            "UPDATE board_column SET on_entry = NULL;
             UPDATE board_column SET movers = '{lead}' WHERE kind = 'done' OR key LIKE '%ready';
             DELETE FROM meta WHERE key = 'board_chain_added';"
        ))
        .unwrap();
}

fn on(db: &Db, p: &str, board_id: &str) -> Vec<BoardColumn> {
    let mut cols: Vec<BoardColumn> = db
        .board_columns(p)
        .unwrap()
        .into_iter()
        .filter(|c| c.board_id == board_id)
        .collect();
    cols.sort_by_key(|c| c.ord);
    cols
}

fn roles(cols: &[BoardColumn]) -> Vec<(String, Option<Role>)> {
    cols.iter().map(|c| (c.key.clone(), c.on_entry)).collect()
}

#[test]
fn boards_made_from_the_two_presets_get_the_chain_once() {
    let (db, p) = board();
    let gated = db.boards(&p).unwrap().remove(0);
    let review = db
        .create_board(&p, "Second", "SR", Preset::SimpleReview)
        .unwrap();
    let software = db.create_board(&p, "Ship", "SW", Preset::Software).unwrap();
    let simple = db.create_board(&p, "Plain", "PL", Preset::Simple).unwrap();
    as_023(&db);
    db.backfill_board_chain().unwrap();

    let cols = on(&db, &p, &review.id);
    assert_eq!(
        roles(&cols)[..5],
        [
            ("todo".to_string(), Some(Role::Product)),
            ("sr-research".to_string(), Some(Role::Architect)),
            ("sr-doing".to_string(), Some(Role::Dev)),
            ("sr-review".to_string(), Some(Role::Tester)),
            ("sr-done".to_string(), None),
        ]
    );
    let tester = Mover::Roles(vec!["tester".into()]);
    assert_eq!(cols[4].policy.movers, vec![Mover::Lead, tester.clone()]);

    let cols = on(&db, &p, &software.id);
    let keys: Vec<(&str, Option<Role>)> =
        cols.iter().map(|c| (c.key.as_str(), c.on_entry)).collect();
    assert_eq!(keys[0], ("sw-inbox", Some(Role::Product)));
    assert_eq!(keys[1], ("sw-ready", None));
    assert_eq!(
        cols[1].policy.movers,
        vec![Mover::Lead, Mover::Roles(vec!["product".into()])]
    );
    assert_eq!(cols[5].policy.movers, vec![Mover::Lead, tester]);

    // A release-gated board and a board from another preset are custom
    // work: they start with no role.
    assert!(on(&db, &p, &gated.id).iter().all(|c| c.on_entry.is_none()));
    assert!(on(&db, &p, &simple.id).iter().all(|c| c.on_entry.is_none()));

    // The owner clears a role: it stays cleared.
    db.lock()
        .execute(
            "UPDATE board_column SET on_entry = NULL WHERE key = 'todo'",
            [],
        )
        .unwrap();
    db.backfill_board_chain().unwrap();
    assert_eq!(on(&db, &p, &review.id)[0].on_entry, None);
}
