use super::sent_back;
use super::text::{text, Route};
use crate::kanban::model::{
    BoardColumn, ColumnCategory, ColumnKind, Item, ItemState, ItemType, Priority, Role,
};
use crate::kanban::presets::{Preset, Start};

/// The columns a Simple + Review board starts with, as stored.
fn columns() -> Vec<BoardColumn> {
    let start = Start::of(Preset::SimpleReview);
    start
        .columns
        .iter()
        .zip(&start.categories)
        .enumerate()
        .map(|(ord, (c, category))| BoardColumn {
            project_id: "p".into(),
            board_id: "b".into(),
            key: c.key.clone(),
            name: c.name.clone(),
            ord: ord as u32,
            category: *category,
            kind: c.kind,
            wip_limit: c.wip_limit,
            wip_scope: c.wip_scope,
            visible: c.visible,
            policy: c.policy.clone(),
            on_entry: c.on_entry,
        })
        .collect()
}

fn item() -> Item {
    let now = chrono::Utc::now();
    Item {
        id: "F-7".into(),
        board_id: "b".into(),
        seq: 7,
        item_type: ItemType::Task,
        title: "Export".into(),
        description: String::new(),
        size: None,
        priority: Priority::P2,
        rank: "a".into(),
        column_key: "doing".into(),
        category: ColumnCategory::Doing,
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
        plan_node_id: None,
    }
}

fn column<'a>(columns: &'a [BoardColumn], key: &str) -> &'a BoardColumn {
    columns.iter().find(|c| c.key == key).expect("column")
}

#[test]
fn the_first_board_runs_the_default_chain() {
    let columns = columns();
    let roles: Vec<(&str, Option<Role>)> = columns
        .iter()
        .map(|c| (c.key.as_str(), c.on_entry))
        .collect();
    assert_eq!(
        roles,
        vec![
            ("todo", Some(Role::Product)),
            ("research", Some(Role::Architect)),
            ("doing", Some(Role::Dev)),
            ("review", Some(Role::Tester)),
            ("done", None),
            ("cancelled", None),
        ]
    );
    let done: Vec<String> = column(&columns, "done")
        .policy
        .movers
        .iter()
        .map(|m| m.token())
        .collect();
    assert_eq!(done, vec!["lead", "roles:tester"]);
}

#[test]
fn only_a_move_back_from_testing_is_sent_back() {
    let columns = columns();
    let (doing, review, done) = (
        column(&columns, "doing"),
        column(&columns, "review"),
        column(&columns, "done"),
    );
    assert!(sent_back(review, doing));
    assert!(!sent_back(doing, review));
    assert!(!sent_back(done, review), "Done assigns nobody");
    let todo = column(&columns, "todo");
    assert!(!sent_back(review, todo), "To do is not active work");
    let mut verify = review.clone();
    verify.on_entry = None;
    verify.category = ColumnCategory::Verify;
    assert!(sent_back(&verify, doing));
}

#[test]
fn a_tester_is_told_where_a_pass_and_a_failure_go() {
    let columns = columns();
    let review = column(&columns, "review");
    let item = item();
    let route = Route::of(review, &columns);
    let text = text(&item, &route, Role::Tester, Some("Ada"), true, None);
    assert!(text.starts_with("[card F-7] Ada moved F-7 “Export” to Review"));
    assert!(text.contains("as the project's Tester"));
    assert!(text.contains("If it passes, move it to Done."));
    assert!(text.contains("move it back to Doing with item_move and a reason"));
    assert_eq!(route.column.kind, ColumnKind::Active);
}

#[test]
fn a_developer_sent_back_reads_the_reason() {
    let columns = columns();
    let doing = column(&columns, "doing");
    let item = item();
    let route = Route::of(doing, &columns);
    let text = text(
        &item,
        &route,
        Role::Dev,
        None,
        true,
        Some("CSV has no header"),
    );
    assert!(text.starts_with("[card F-7] F-7 “Export” moved to Doing, where the Developer"));
    assert!(text.contains("It was sent back: “CSV has no header”."));
    assert!(text.contains("Then move it to Review with item_move."));
}
