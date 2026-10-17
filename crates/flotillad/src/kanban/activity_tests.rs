//! What one board change announces, from the history it recorded.

use super::*;

fn event(id: i64, kind: ItemEventKind, actor: &str) -> ItemEvent {
    ItemEvent {
        id,
        item_id: "P-1".into(),
        at: chrono::Utc::now(),
        actor: actor.into(),
        kind,
        from: None,
        to: None,
        field: None,
        note: None,
    }
}

fn change(kind: ChangeKind, events: Vec<ItemEvent>) -> BoardChange {
    BoardChange {
        project_id: "p".into(),
        seq: 1,
        kind,
        item_id: "P-1".into(),
        card: None,
        from_column: None,
        board_id: None,
        actor: events.last().map(|e| e.actor.clone()),
        note: None,
        events,
        origin: None,
        plan: None,
    }
}

fn kinds(out: &[BoardActivity]) -> Vec<(Kind, &str)> {
    out.iter()
        .map(|a| (Kind::try_from(a.kind).unwrap(), a.actor.as_str()))
        .collect()
}

#[test]
fn a_move_and_an_assignment_each_say_who() {
    let moved = ItemEvent {
        from: Some("todo".into()),
        to: Some("doing".into()),
        note: Some("started".into()),
        ..event(1, ItemEventKind::Moved, "bot:b1")
    };
    let assigned = ItemEvent {
        to: Some("b2".into()),
        ..event(2, ItemEventKind::Assigned, "user")
    };
    let unassigned = ItemEvent {
        from: Some("b2".into()),
        ..event(3, ItemEventKind::Assigned, "bot:lead")
    };
    let out = activities(&change(
        ChangeKind::ItemMoved,
        vec![moved, assigned, unassigned],
    ));
    assert_eq!(
        kinds(&out),
        [
            (Kind::Moved, "bot:b1"),
            (Kind::Assigned, "user"),
            (Kind::Unassigned, "bot:lead")
        ]
    );
    assert_eq!(
        (out[0].from_column.as_str(), out[0].to_column.as_str()),
        ("todo", "doing")
    );
    assert_eq!(out[0].note, "started");
    assert_eq!(
        (out[1].assignee.as_str(), out[2].assignee.as_str()),
        ("b2", "b2")
    );
}

#[test]
fn a_facts_move_is_the_daemons_whoever_reported_it() {
    let mut c = change(
        ChangeKind::ItemMoved,
        vec![event(1, ItemEventKind::Moved, "user")],
    );
    c.actor = Some(DAEMON.into());
    assert_eq!(kinds(&activities(&c)), [(Kind::Moved, DAEMON)]);
}

#[test]
fn a_relayed_move_says_what_the_home_sent() {
    let mut c = change(ChangeKind::ItemMoved, Vec::new());
    c.actor = Some("bot:b9".into());
    c.from_column = Some("todo".into());
    c.note = Some("picked up".into());
    let out = activities(&c);
    assert_eq!(kinds(&out), [(Kind::Moved, "bot:b9")]);
    assert_eq!(
        (out[0].from_column.as_str(), out[0].note.as_str()),
        ("todo", "picked up")
    );
    // An upsert with no history announces nothing.
    assert!(activities(&change(ChangeKind::ItemUpserted, Vec::new())).is_empty());
}

#[test]
fn a_bots_draft_edits_fold_into_one_and_the_owners_are_their_own() {
    let edit = |id, actor: &str, field: &str| ItemEvent {
        field: Some(field.into()),
        ..event(id, ItemEventKind::Edited, actor)
    };
    let events = vec![
        edit(1, "bot:b1", "description"),
        edit(2, "bot:b1", "acceptance_criteria"),
        edit(3, "bot:b1", "description"),
        edit(4, "bot:b1", "invited"),
    ];
    let out = activities(&change(ChangeKind::DraftChanged, events));
    assert_eq!(kinds(&out), [(Kind::DraftUpdated, "bot:b1")]);
    assert_eq!(out[0].note, "description, acceptance criteria");
    let owners = vec![edit(1, "user", "title")];
    assert!(activities(&change(ChangeKind::DraftChanged, owners)).is_empty());
    // A live card's edits aren't a draft's.
    let live = vec![edit(1, "bot:b1", "title")];
    assert!(activities(&change(ChangeKind::ItemUpserted, live)).is_empty());
}
