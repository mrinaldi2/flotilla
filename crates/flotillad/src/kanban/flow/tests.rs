//! The custom flow: movers, allowed sources, each entry rule, WIP and
//! returns, against the Software preset's columns.

use chrono::Utc;

use super::*;
use crate::kanban::guards::ColumnLoad;
use crate::kanban::model::{
    AcceptanceCriterion, ColumnCategory, ItemLink, ItemType, Priority, Role, WipScope,
};
use crate::kanban::presets::Preset;

fn column(key: &str) -> (BoardColumn, ColumnPolicy) {
    let preset = Preset::Software;
    let (ord, c) = preset
        .columns()
        .into_iter()
        .enumerate()
        .find(|(_, c)| c.key == key)
        .expect("preset column");
    let column = BoardColumn {
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
        policy: c.policy.clone(),
    };
    (column, c.policy)
}

fn item(in_column: &str) -> Item {
    let now = Utc::now();
    Item {
        id: "RB-1".into(),
        board_id: "b".into(),
        seq: 1,
        item_type: ItemType::Feature,
        title: "Tags".into(),
        description: String::new(),
        size: Some(Size::S),
        priority: Priority::P2,
        rank: "a".into(),
        column_key: in_column.into(),
        category: ColumnCategory::Inbox,
        blocked: None,
        assignee: Some("dev".into()),
        parent_id: None,
        release_id: None,
        labels: Vec::new(),
        acceptance_criteria: vec![AcceptanceCriterion {
            idx: 0,
            text: "works".into(),
            checked: false,
            checked_by: None,
            checked_at: None,
            machine: None,
            post_install: false,
        }],
        people: Vec::new(),
        verifications: Vec::new(),
        created_by: "user".into(),
        created_at: now,
        updated_at: now,
        state_entered_at: now,
        done_at: None,
        version: 1,
        state: crate::kanban::model::ItemState::Live,
    }
}

fn bot(id: &str, roles: &[Role]) -> Who {
    Who::Bot {
        id: id.into(),
        roles: roles.to_vec(),
    }
}

fn codes(item: &Item, from: &str, to: &str, who: &Who, ctx: &Context) -> Vec<String> {
    let (from, _) = column(from);
    let (to, policy) = column(to);
    let mv = Move {
        to: &to,
        reason: None,
        override_reason: None,
    };
    evaluate(item, &from, &policy, &mv, who, ctx)
        .into_iter()
        .map(|u| u.code)
        .collect()
}

#[test]
fn ready_needs_the_lead_criteria_a_small_size_and_done_blockers() {
    let lead = bot("lead", &[Role::Lead]);
    let ctx = Context::default();
    assert!(codes(&item("inbox"), "inbox", "ready", &lead, &ctx).is_empty());
    assert_eq!(
        codes(
            &item("inbox"),
            "inbox",
            "ready",
            &bot("dev", &[Role::Dev]),
            &ctx
        ),
        ["role.not_allowed"]
    );
    let mut big = item("inbox");
    big.size = Some(Size::L);
    big.acceptance_criteria.clear();
    assert_eq!(
        codes(&big, "inbox", "ready", &lead, &ctx),
        ["rule.ac_present", "rule.size_at_most"]
    );
    // The owner may move anything a rule allows; the rules still hold.
    assert_eq!(
        codes(&big, "inbox", "ready", &Who::Owner, &ctx),
        ["rule.ac_present", "rule.size_at_most"]
    );
}

#[test]
fn review_comes_only_from_doing_with_a_pr_by_the_assignee() {
    let dev = bot("dev", &[Role::Dev]);
    let none = Context::default();
    assert_eq!(
        codes(&item("doing"), "doing", "review", &dev, &none),
        ["rule.link"]
    );
    let with_pr = Context {
        links: vec![ItemLink {
            item_id: "RB-1".into(),
            kind: LinkKind::Pr,
            target: "#4".into(),
            label: None,
            created_by: "bot:dev".into(),
            at: Utc::now(),
        }],
        ..Context::default()
    };
    assert!(codes(&item("doing"), "doing", "review", &dev, &with_pr).is_empty());
    assert_eq!(
        codes(&item("ready"), "ready", "review", &dev, &with_pr),
        ["move.from"]
    );
    assert_eq!(
        codes(
            &item("doing"),
            "doing",
            "review",
            &bot("other", &[]),
            &with_pr
        ),
        ["role.not_allowed"]
    );
}

#[test]
fn wip_refuses_a_move_but_never_a_return() {
    let lead = bot("lead", &[Role::Lead]);
    let mut ctx = Context::default();
    ctx.load.insert(
        "review".into(),
        ColumnLoad {
            items: 3,
            assignee_items: 0,
        },
    );
    ctx.links.push(ItemLink {
        item_id: "RB-1".into(),
        kind: LinkKind::Pr,
        target: "#4".into(),
        label: None,
        created_by: "bot:dev".into(),
        at: Utc::now(),
    });
    let dev = bot("dev", &[Role::Dev]);
    assert_eq!(
        codes(&item("doing"), "doing", "review", &dev, &ctx),
        ["wip.full"]
    );
    ctx.load.insert(
        "doing".into(),
        ColumnLoad {
            items: 5,
            assignee_items: 5,
        },
    );
    // Back from Review to Doing is a return.
    let (from, _) = column("review");
    let (to, _) = column("doing");
    assert!(is_return(&from, &to));
    assert!(codes(&item("review"), "review", "doing", &lead, &ctx).is_empty());
}

#[test]
fn cancelling_needs_the_lead_and_a_reason() {
    let (from, _) = column("doing");
    let (to, policy) = column("cancelled");
    assert_eq!(to.wip_scope, WipScope::Column);
    let mv = |reason| Move {
        to: &to,
        reason,
        override_reason: None,
    };
    let lead = bot("lead", &[Role::Lead]);
    let ctx = Context::default();
    let refused = evaluate(&item("doing"), &from, &policy, &mv(None), &lead, &ctx);
    assert_eq!(refused[0].code, "reason.required");
    assert!(evaluate(
        &item("doing"),
        &from,
        &policy,
        &mv(Some("dup")),
        &lead,
        &ctx
    )
    .is_empty());
}

#[test]
fn only_automations_move_into_a_system_column() {
    let (from, _) = column("doing");
    let (to, _) = column("review");
    let policy = ColumnPolicy {
        movers: vec![Mover::System],
        ..ColumnPolicy::default()
    };
    let mv = Move {
        to: &to,
        reason: None,
        override_reason: None,
    };
    let ctx = Context::default();
    for who in [Who::Owner, bot("lead", &[Role::Lead])] {
        let refused = evaluate(&item("doing"), &from, &policy, &mv, &who, &ctx);
        assert_eq!(refused[0].code, "role.not_allowed", "{who:?}");
        assert!(refused[0].text.contains("automations"));
    }
    assert!(evaluate(&item("doing"), &from, &policy, &mv, &Who::Daemon, &ctx).is_empty());
}

#[test]
fn every_rule_round_trips_through_json() {
    let rules = vec![
        EntryRule::AssigneeSet,
        EntryRule::FieldsFilled {
            sections: vec!["Goal".into()],
        },
        EntryRule::AcPresent,
        EntryRule::AcAllChecked,
        EntryRule::SizeAtMost { size: Size::M },
        EntryRule::BlockersDone,
        EntryRule::NotBlocked,
        EntryRule::Link { kind: "pr".into() },
        EntryRule::Independent,
        EntryRule::Reason,
        EntryRule::Predicate {
            name: "code.linked".into(),
        },
    ];
    let text = serde_json::to_string(&rules).expect("json");
    assert!(
        text.contains(r#"{"rule":"size_at_most","size":"M"}"#),
        "{text}"
    );
    let back: Vec<EntryRule> = serde_json::from_str(&text).expect("parse");
    assert_eq!(back, rules);
    let movers = vec![Mover::Lead, Mover::Roles(vec!["tester".into()])];
    let text = serde_json::to_string(&movers).expect("json");
    assert_eq!(text, r#"["lead",{"roles":["tester"]}]"#);
}
