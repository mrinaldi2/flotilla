//! The first board's flow (Simple + Review): the assignee works its card
//! To do ↔ Doing → Review, and Done is the owner's, or the lead's when the
//! lead isn't the assignee.

use chrono::Utc;

use super::*;
use crate::kanban::guards::Context;
use crate::kanban::model::{ColumnCategory, ItemState, ItemType, Priority, Role};
use crate::kanban::presets::Preset;

fn column(key: &str) -> BoardColumn {
    let (ord, c) = Preset::SimpleReview
        .columns()
        .into_iter()
        .enumerate()
        .find(|(_, c)| c.key == key)
        .expect("preset column");
    BoardColumn {
        project_id: "p".into(),
        board_id: "b".into(),
        category: c.category(),
        key: c.key,
        name: c.name,
        ord: ord as u32,
        kind: c.kind,
        wip_limit: c.wip_limit,
        wip_scope: c.wip_scope,
        visible: c.visible,
        policy: c.policy,
    }
}

fn item(in_column: &str, assignee: Option<&str>) -> Item {
    let now = Utc::now();
    Item {
        id: "P-1".into(),
        board_id: "b".into(),
        seq: 1,
        item_type: ItemType::Feature,
        title: "Buttons".into(),
        description: String::new(),
        size: None,
        priority: Priority::P2,
        rank: "a".into(),
        column_key: in_column.into(),
        category: ColumnCategory::Inbox,
        blocked: None,
        assignee: assignee.map(str::to_string),
        parent_id: None,
        release_id: None,
        labels: Vec::new(),
        acceptance_criteria: Vec::new(),
        people: Vec::new(),
        verifications: Vec::new(),
        created_by: "user".into(),
        created_at: now,
        updated_at: now,
        state_entered_at: now,
        done_at: None,
        version: 1,
        state: ItemState::Live,
    }
}

fn bot(id: &str, roles: &[Role]) -> Who {
    Who::Bot {
        id: id.into(),
        roles: roles.to_vec(),
    }
}

fn codes(item: &Item, to: &str, who: &Who) -> Vec<String> {
    let from = column(&item.column_key);
    let to = column(to);
    let mv = Move {
        to: &to,
        reason: None,
        override_reason: None,
    };
    evaluate(item, &from, &to.policy, &mv, who, &Context::default())
        .into_iter()
        .map(|u| u.code)
        .collect()
}

#[test]
fn the_assignee_works_its_card_to_review_without_a_role() {
    let me = bot("helper", &[]);
    let mine = |col| item(col, Some("helper"));
    assert!(codes(&mine("todo"), "doing", &me).is_empty());
    assert!(codes(&mine("doing"), "todo", &me).is_empty());
    assert!(codes(&mine("doing"), "review", &me).is_empty());
    assert_eq!(codes(&mine("todo"), "review", &me), ["move.from"]);
    // Someone else's card is not its to move.
    let theirs = item("todo", Some("other"));
    assert_eq!(codes(&theirs, "doing", &me), ["role.not_allowed"]);
    // Doing needs an assignee, whoever moves it.
    assert_eq!(
        codes(&item("todo", None), "doing", &Who::Owner),
        ["rule.assignee_set"]
    );
}

#[test]
fn done_is_the_owners_or_an_independent_leads() {
    let card = item("review", Some("helper"));
    assert!(codes(&card, "done", &Who::Owner).is_empty());
    assert!(codes(&card, "done", &bot("lead", &[Role::Lead])).is_empty());
    assert_eq!(
        codes(&card, "done", &bot("helper", &[])),
        ["role.not_allowed", "rule.independent"]
    );
    // A lead that did the work doesn't approve it.
    let own = item("review", Some("lead"));
    assert_eq!(
        codes(&own, "done", &bot("lead", &[Role::Lead])),
        ["rule.independent"]
    );
}

#[test]
fn a_lead_that_held_the_card_doesnt_approve_it_once_it_hands_it_on() {
    let from = column("review");
    let to = column("done");
    let mv = Move {
        to: &to,
        reason: None,
        override_reason: None,
    };
    let ctx = Context {
        past_assignees: vec!["lead".into()],
        ..Context::default()
    };
    let lead = bot("lead", &[Role::Lead]);
    for assignee in [None, Some("helper")] {
        let card = item("review", assignee);
        let unmet: Vec<String> = evaluate(&card, &from, &to.policy, &mv, &lead, &ctx)
            .into_iter()
            .map(|u| u.code)
            .collect();
        assert_eq!(unmet, ["rule.independent"], "assignee {assignee:?}");
    }
    assert!(evaluate(
        &item("review", None),
        &from,
        &to.policy,
        &mv,
        &Who::Owner,
        &ctx
    )
    .is_empty());
}
