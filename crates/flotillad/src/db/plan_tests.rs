//! Plan storage (0.21.0): the switch, node ids across projects, the tree's
//! order, versions, gates, history, and the cards and spikes of a node.

use crate::kanban::model::{ItemEventKind, ItemType, LinkKind};
use crate::plan::model::{Child, DocKind, Gate, Kind, NewNode, ResearchState, Stage};

use super::board_edit::ItemEdit;
use super::board_items::Write;
use super::board_tests::{board, new_item, OWNER};
use super::Db;

fn node<'a>(
    project_id: &'a str,
    kind: Kind,
    parent: Option<&'a str>,
    title: &'a str,
) -> NewNode<'a> {
    NewNode {
        project_id,
        kind,
        parent_id: parent,
        title,
        summary: "",
        stage: Stage::Define,
        due: None,
        lead: None,
        board_id: None,
        research: kind.researches().then_some(ResearchState::Open),
    }
}

fn insert(db: &Db, new: &NewNode<'_>) -> String {
    db.board_tx(|t| t.insert_plan_node(new, "user")).unwrap().id
}

#[test]
fn planning_is_off_until_turned_on_and_off_keeps_the_switch_row() {
    let (db, p) = board();
    assert_eq!(db.board_read(|t| t.plan_settings(&p)).unwrap(), None);
    db.board_tx(|t| t.set_plan_enabled(&p, true)).unwrap();
    let on = db.board_read(|t| t.plan_settings(&p)).unwrap().unwrap();
    assert!(on.enabled);
    assert_eq!(on.next_seq, 1);
    db.board_tx(|t| t.set_plan_enabled(&p, false)).unwrap();
    assert!(
        !db.board_read(|t| t.plan_settings(&p))
            .unwrap()
            .unwrap()
            .enabled
    );
}

#[test]
fn node_ids_count_per_project_and_stay_unique_across_projects() {
    let (db, p) = board();
    db.board_tx(|t| t.set_plan_enabled(&p, true)).unwrap();
    let first = insert(&db, &node(&p, Kind::Goal, None, "Ship the demo"));
    let second = insert(&db, &node(&p, Kind::Goal, None, "Playtest"));
    assert_eq!((first.as_str(), second.as_str()), ("PL-1", "PL-2"));

    let other = db.create_project("Game", "game").unwrap();
    db.ensure_board(&other.id, "d-mac", Some("GM")).unwrap();
    db.board_tx(|t| t.set_plan_enabled(&other.id, true))
        .unwrap();
    let theirs = insert(&db, &node(&other.id, Kind::Goal, None, "Launch"));
    assert_eq!(theirs, "GM-PL-1", "PL-1 is the first project's");
    let next = insert(&db, &node(&other.id, Kind::Goal, None, "Grow"));
    assert_eq!(next, "GM-PL-2");
}

#[test]
fn the_tree_lists_parents_before_children_by_rank() {
    let (db, p) = board();
    db.board_tx(|t| t.set_plan_enabled(&p, true)).unwrap();
    let g1 = insert(&db, &node(&p, Kind::Goal, None, "One"));
    let g2 = insert(&db, &node(&p, Kind::Goal, None, "Two"));
    let i1 = insert(&db, &node(&p, Kind::Initiative, Some(&g1), "Under one"));
    let e1 = insert(&db, &node(&p, Kind::Epic, Some(&i1), "Epic"));
    let i2 = insert(
        &db,
        &node(&p, Kind::Initiative, Some(&g1), "Also under one"),
    );
    let ids: Vec<String> = db
        .board_read(|t| t.plan_nodes(&p))
        .unwrap()
        .into_iter()
        .map(|n| n.id)
        .collect();
    assert_eq!(ids, [g1.clone(), i1.clone(), e1.clone(), i2.clone(), g2]);
    let under = db.board_read(|t| t.plan_subtree(&g1)).unwrap();
    assert_eq!(under, [g1, i1, i2, e1]);
}

#[test]
fn a_stale_version_claims_nothing() {
    let (db, p) = board();
    db.board_tx(|t| t.set_plan_enabled(&p, true)).unwrap();
    let id = insert(&db, &node(&p, Kind::Goal, None, "Goal"));
    assert!(db
        .board_tx(|t| t.claim_plan_node(&id, Some(1)))
        .unwrap()
        .is_none());
    let now = db
        .board_tx(|t| t.claim_plan_node(&id, Some(1)))
        .unwrap()
        .expect("stale");
    assert_eq!(now.version, 2);
}

#[test]
fn a_gate_keeps_its_children_and_rounds_count_submissions() {
    let (db, p) = board();
    db.board_tx(|t| t.set_plan_enabled(&p, true)).unwrap();
    let id = insert(&db, &node(&p, Kind::Initiative, None, "Init"));
    let gate = Gate {
        stage: Stage::Prd,
        doc_path: Some("docs/i/prd.md".into()),
        sha: Some("abc1234".into()),
        pr_number: Some(7),
        submitted_by: "bot:lead".into(),
        submitted_at: bus::now(),
        note: None,
        children: vec![
            Child {
                title: "Epic A".into(),
                summary: Some("Why".into()),
            },
            Child {
                title: "Epic B".into(),
                summary: None,
            },
        ],
        decision_id: None,
        round: 1,
    };
    db.board_tx(|t| {
        t.set_plan_gate(&id, &gate)?;
        t.upsert_plan_doc(&id, DocKind::Prd, "docs/i/prd.md", None)?;
        t.record_plan_event(&id, "bot:lead", "submitted", Some(Stage::Prd), None)
    })
    .unwrap();
    let read = db.board_read(|t| t.plan_node(&id)).unwrap().unwrap();
    let stored = read.gate.expect("a gate");
    assert_eq!(stored.children, gate.children);
    assert_eq!((stored.pr_number, stored.round), (Some(7), 1));
    assert_eq!(read.docs[0].path, "docs/i/prd.md");
    assert_eq!(
        db.board_read(|t| t.plan_rounds(&id, Stage::Prd)).unwrap(),
        1
    );
    assert_eq!(
        db.board_read(|t| t.plan_rounds(&id, Stage::Hld)).unwrap(),
        0
    );
    db.board_tx(|t| t.approve_plan_doc(&id, DocKind::Prd, "docs/i/prd.md", Some("abc1234")))
        .unwrap();
    db.board_tx(|t| t.clear_plan_gate(&id)).unwrap();
    let read = db.board_read(|t| t.plan_node(&id)).unwrap().unwrap();
    assert!(read.gate.is_none());
    assert_eq!(read.docs[0].approved_sha.as_deref(), Some("abc1234"));
    let events = db.board_read(|t| t.plan_events(&id)).unwrap();
    assert_eq!(events[0].kind, "submitted");
}

#[test]
fn a_nodes_cards_and_spikes_are_its_own() {
    let (db, p) = board();
    db.board_tx(|t| t.set_plan_enabled(&p, true)).unwrap();
    let id = insert(&db, &node(&p, Kind::Initiative, None, "Init"));
    let spike = db
        .create_item(
            &super::NewItem {
                item_type: ItemType::Spike,
                plan_node_id: Some(&id),
                ..new_item(&p, "Look around")
            },
            &OWNER,
        )
        .unwrap();
    assert_eq!(spike.plan_node_id.as_deref(), Some(id.as_str()));
    let other = db.create_item(&new_item(&p, "Elsewhere"), &OWNER).unwrap();
    let read = db.board_read(|t| t.plan_node(&id)).unwrap().unwrap();
    assert_eq!((read.cards_total, read.cards_done), (1, 0));
    assert_eq!(
        read.research.expect("research").spikes,
        std::slice::from_ref(&spike.id)
    );
    let spikes = db.board_read(|t| t.plan_spikes(&id)).unwrap();
    assert!(!spikes[0].done && spikes[0].docs.is_empty());
    db.add_item_link(
        &spike.id,
        LinkKind::Artifact,
        "research/notes.md",
        None,
        &OWNER,
    )
    .unwrap();
    let spikes = db.board_read(|t| t.plan_spikes(&id)).unwrap();
    assert_eq!(spikes[0].docs, ["research/notes.md"]);
    let cards = db.board_read(|t| t.plan_cards(&p, &id)).unwrap();
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].plan_node_id.as_deref(), Some(id.as_str()));

    // A card moves to the node, and off it, with a line of history.
    let edit = ItemEdit {
        plan_node_id: Some(Some(&id)),
        ..ItemEdit::default()
    };
    let Write::Done(moved) = db
        .update_item(&other.id, other.version, &edit, &OWNER)
        .unwrap()
    else {
        panic!("not a conflict");
    };
    assert_eq!(moved.plan_node_id.as_deref(), Some(id.as_str()));
    let clear = ItemEdit {
        plan_node_id: Some(None),
        ..ItemEdit::default()
    };
    let Write::Done(cleared) = db
        .update_item(&other.id, moved.version, &clear, &OWNER)
        .unwrap()
    else {
        panic!("not a conflict");
    };
    assert_eq!(cleared.plan_node_id, None);
    let fields: Vec<Option<String>> = db
        .item_events(&other.id)
        .unwrap()
        .into_iter()
        .filter(|e| e.kind == ItemEventKind::Edited)
        .map(|e| e.field)
        .collect();
    assert_eq!(
        fields,
        [Some("plan_node_id".into()), Some("plan_node_id".into())]
    );
}
