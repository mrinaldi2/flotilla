//! The assignee's notices, on the first board's columns and on a
//! release-gated board.

use std::collections::BTreeMap;

use chrono::Utc;

use super::*;
use crate::kanban::model::{ItemState, ItemType, Priority, StructurePolicy};
use crate::kanban::presets::Preset;

fn columns(preset: Preset) -> Vec<BoardColumn> {
    preset
        .columns()
        .into_iter()
        .zip(preset.categories())
        .enumerate()
        .map(|(ord, (c, category))| BoardColumn {
            project_id: "p".into(),
            board_id: "b".into(),
            key: c.key.into(),
            name: c.name.into(),
            ord: ord as u32,
            category,
            kind: c.kind,
            wip_limit: c.wip_limit,
            wip_scope: c.wip_scope,
            visible: c.visible,
            policy: c.policy,
        })
        .collect()
}

fn board(preset: Preset) -> Board {
    Board {
        id: "b".into(),
        project_id: "p".into(),
        name: "Main".into(),
        key: "P".into(),
        description: String::new(),
        ord: 0,
        is_default: true,
        structure_policy: StructurePolicy::OwnerConfirms,
        flow: preset.flow(),
        automations: preset.automations().into_iter().collect::<BTreeMap<_, _>>(),
        label_groups: Vec::new(),
        archived_at: None,
        version: 1,
    }
}

fn item(column: &BoardColumn) -> Item {
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
        column_key: column.key.clone(),
        category: column.category,
        blocked: None,
        assignee: Some("bot-1".into()),
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

fn notice(preset: Preset, key: &str, n: Notice<'_>, last: Option<&str>) -> String {
    let cols = columns(preset);
    let column = cols.iter().find(|c| c.key == key).expect("column");
    let it = item(column);
    let b = board(preset);
    text(
        &Card {
            item: &it,
            by: "The owner",
            column,
            plan: Plan::of(Some(&b), &cols),
            last_comment: last,
        },
        n,
    )
}

#[test]
fn the_first_board_plans_doing_review_and_back_to_to_do() {
    let cols = columns(Preset::SimpleReview);
    let plan = Plan::of(Some(&board(Preset::SimpleReview)), &cols).expect("custom");
    assert_eq!(plan.start, Some(("Doing".to_string(), true)));
    assert_eq!(plan.finish.as_deref(), Some("Review"));
    assert_eq!(plan.back.as_deref(), Some("To do"));
    let simple = Plan::of(Some(&board(Preset::Simple)), &columns(Preset::Simple)).expect("custom");
    assert_eq!(simple.start, Some(("Doing".to_string(), false)));
    assert_eq!(simple.finish.as_deref(), Some("Done"));
    let gated = board(Preset::ReleaseGated);
    assert!(Plan::of(Some(&gated), &columns(Preset::ReleaseGated)).is_none());
}

#[test]
fn an_assignment_says_which_card_where_and_what_to_do() {
    let text = notice(Preset::SimpleReview, "todo", Notice::Assigned, None);
    assert!(
        text.starts_with("[card P-1] The owner assigned P-1 “Buttons” to you."),
        "{text}"
    );
    assert!(text.contains("It is in To do (not started)."), "{text}");
    assert!(
        text.contains("Start now: read it with item_get, which moves it to Doing."),
        "{text}"
    );
    assert!(text.contains("move it to Review with item_move"), "{text}");
    assert!(text.contains("after moving it back to To do"), "{text}");
    assert!(
        text.contains("message_owner (asks: true, item: P-1)"),
        "{text}"
    );
}

#[test]
fn on_a_board_without_the_automation_the_bot_moves_it_itself() {
    let text = notice(Preset::Simple, "todo", Notice::Assigned, None);
    assert!(text.contains("move it to Doing with item_move"), "{text}");
    assert!(text.contains("move it to Done with item_move"), "{text}");
}

#[test]
fn a_release_gated_assignment_points_at_its_rules() {
    let text = notice(Preset::ReleaseGated, "inbox", Notice::Assigned, None);
    assert!(text.contains("It is in Inbox (not started)."), "{text}");
    assert!(
        text.contains("item_move_check shows which moves are yours"),
        "{text}"
    );
    assert!(!text.contains("Start now"), "{text}");
}

#[test]
fn a_move_after_the_work_is_done_is_news_not_work() {
    let done = Some("Done: the page is at artifacts/p-1.html");
    let text = notice(Preset::SimpleReview, "todo", Notice::Moved, done);
    assert!(
        text.contains("moved it to To do. It is now in To do (not started)."),
        "{text}"
    );
    assert!(text.contains("This is a notice, not new work"), "{text}");
    assert!(text.contains("don't redo it"), "{text}");
    assert!(
        text.contains("Your last comment on it: “Done: the page is at"),
        "{text}"
    );
    let closed = notice(Preset::SimpleReview, "done", Notice::Moved, None);
    assert!(
        closed.contains("It is done: there is nothing to do."),
        "{closed}"
    );
}

#[test]
fn an_edit_and_an_unassignment_say_what_changed() {
    let edited = notice(
        Preset::SimpleReview,
        "doing",
        Notice::Changed("edited it"),
        None,
    );
    assert!(
        edited.contains("which is assigned to you: edited it."),
        "{edited}"
    );
    assert!(
        edited.contains("It is now in Doing (in progress)."),
        "{edited}"
    );
    let off = notice(Preset::SimpleReview, "doing", Notice::Unassigned, None);
    assert!(off.contains("took P-1 “Buttons” off you"), "{off}");
}

#[test]
fn a_long_comment_is_quoted_short() {
    let long = "word ".repeat(100);
    let text = notice(Preset::SimpleReview, "review", Notice::Moved, Some(&long));
    assert!(text.ends_with("…”"), "{text}");
    assert!(text.len() < 900, "{}", text.len());
}
