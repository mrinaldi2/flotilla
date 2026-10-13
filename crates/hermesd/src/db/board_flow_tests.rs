//! A custom board next to a release-gated one (0.19.0 K3): its columns,
//! rules and automations decide, and facts reach both boards their own way.

use super::board_tests::{board, new_item};
use super::{Db, NewItem};
use crate::actor::Actor;
use crate::kanban::api::report_fact;
use crate::kanban::facts::Fact;
use crate::kanban::model::{BoardFlow, LinkKind};
use crate::kanban::moves::{item_move, MoveRequest, Moved};
use crate::kanban::presets::Preset;

const OWNER: Actor<'static> = Actor::User;

fn lead_of(db: &Db, p: &str) -> String {
    db.lock()
        .query_row("SELECT lead_bot_id FROM project WHERE id = ?1", [p], |r| {
            r.get(0)
        })
        .unwrap()
}

fn mv(db: &Db, id: &str, to: &str, actor: &Actor<'_>) -> Moved {
    let version = db.get_item(id).unwrap().unwrap().version;
    item_move(
        db,
        &MoveRequest {
            id,
            to,
            expected_version: version,
            reason: None,
            override_reason: None,
        },
        actor,
    )
    .unwrap()
}

fn codes(moved: Moved) -> Vec<String> {
    match moved {
        Moved::Refused(unmet) => unmet.into_iter().map(|u| u.code).collect(),
        other => panic!("not refused: {other:?}"),
    }
}

#[test]
fn a_second_board_gets_its_own_columns_and_automations() {
    let (db, p) = board();
    let research = db
        .create_board(&p, "Research", "RS", Preset::Software)
        .unwrap();
    assert_eq!(research.flow, BoardFlow::Custom);
    assert!(!research.is_default);
    assert_eq!(research.automations["pr.merged"], "rs-done");
    let keys: Vec<String> = db
        .board_columns(&p)
        .unwrap()
        .into_iter()
        .filter(|c| c.board_id == research.id)
        .map(|c| c.key)
        .collect();
    assert_eq!(
        keys,
        [
            "rs-inbox",
            "rs-ready",
            "rs-doing",
            "rs-review",
            "rs-done",
            "rs-cancelled"
        ]
    );
    assert_eq!(db.boards(&p).unwrap().len(), 2);
    // The default board's columns are untouched.
    assert!(db
        .board_columns(&p)
        .unwrap()
        .iter()
        .any(|c| c.key == "inbox"));
}

#[test]
fn a_custom_boards_columns_decide_its_moves_and_facts() {
    let (db, p) = board();
    let rs = db
        .create_board(&p, "Research", "RS", Preset::Software)
        .unwrap();
    let item = db
        .create_item(
            &NewItem {
                board_id: Some(&rs.id),
                acceptance_criteria: &[],
                ..new_item(&p, "Compare parsers")
            },
            &OWNER,
        )
        .unwrap();
    assert!(item.id.starts_with("RS-"));
    assert_eq!(item.column_key, "rs-inbox");
    let lead = lead_of(&db, &p);
    let as_lead = Actor::Bot {
        id: &lead,
        project_id: &p,
    };
    // A column of another board is not this board's.
    assert_eq!(codes(mv(&db, &item.id, "ready", &as_lead)), ["move.column"]);
    assert_eq!(
        codes(mv(&db, &item.id, "rs-ready", &as_lead)),
        ["rule.ac_present"]
    );
    db.lock()
        .execute(
            "INSERT INTO item_ac(item_id, idx, text) VALUES (?1, 0, 'it compares')",
            [&item.id],
        )
        .unwrap();
    assert!(matches!(
        mv(&db, &item.id, "rs-ready", &as_lead),
        Moved::Done(_)
    ));

    // A fact the board maps moves the item; one it doesn't is kept.
    db.board_tx(|t| {
        report_fact(
            t,
            &p,
            &item.id,
            Fact::PrOpened,
            "PR #1 opened",
            false,
            &OWNER,
        )
    })
    .unwrap();
    assert_eq!(
        db.get_item(&item.id).unwrap().unwrap().column_key,
        "rs-review"
    );
    let moved = db
        .board_tx(|t| {
            report_fact(
                t,
                &p,
                &item.id,
                Fact::ReleaseApproved,
                "shipped",
                false,
                &OWNER,
            )
        })
        .unwrap();
    assert_eq!(moved, None);
    assert_eq!(
        db.get_item(&item.id).unwrap().unwrap().column_key,
        "rs-review"
    );
    let facts = db.board_read(|t| t.item_facts(&item.id)).unwrap();
    assert_eq!(facts, ["release.approved"]);
}

#[test]
fn a_release_gated_board_takes_facts_the_way_0_18_moved_items() {
    let (db, p) = board();
    let item = db.create_item(&new_item(&p, "Board"), &OWNER).unwrap();
    db.board_tx(|t| {
        crate::kanban::api::link(t, &item.id, LinkKind::Branch, "feat", None, &OWNER)?;
        report_fact(
            t,
            &p,
            &item.id,
            Fact::PrOpened,
            "PR #2 opened",
            false,
            &OWNER,
        )
    })
    .unwrap();
    assert_eq!(db.get_item(&item.id).unwrap().unwrap().column_key, "review");
}
