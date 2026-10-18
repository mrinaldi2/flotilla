//! The lead's kickoff cards (0.21.0): a plan node waiting on the lead gives
//! it a card assigned to it, which is how a bot starts work, and the card
//! closes once the plan moves on.

use crate::common;

use bus::contract::board as c;
use common::plan::*;
use flotillad::db::Db;
use flotillad::kanban::model::{ColumnCategory, Item, ItemType, Role};
use flotillad::plan::model::{Node, Stage};

/// The bodies of the messages in `bot`'s conversation with the owner.
fn told(db: &Db, bot: &str) -> Vec<String> {
    let Some(conv) = db.dm_conversation(bot).unwrap() else {
        return Vec::new();
    };
    db.list_messages(&conv.id, None, 100)
        .unwrap()
        .into_iter()
        .map(|m| m.body)
        .collect()
}

fn item(plan: &Plan, id: &str) -> Item {
    plan.d.app.db.get_item(id).unwrap().expect("an item")
}

fn closed(plan: &Plan, id: &str) -> bool {
    let item = item(plan, id);
    plan.d
        .app
        .db
        .board_columns(&plan.pid)
        .unwrap()
        .into_iter()
        .any(|c| c.key == item.column_key && c.category == ColumnCategory::Done)
}

fn kickoffs(plan: &Plan, node: &str) -> Vec<String> {
    plan.d
        .app
        .db
        .board_read(|t| t.plan_node_cards(node))
        .unwrap()
        .into_iter()
        .map(|c| c.title)
        .collect()
}

fn activate(plan: &Plan, goal: &Node) -> Node {
    done(plan.owner().update(&c::PlanUpdate {
        id: goal.id.clone(),
        expected_version: goal.version,
        stage: Some(c::PlanStage::Active as i32),
        ..Default::default()
    }))
}

#[tokio::test]
async fn an_active_goal_asks_the_lead_for_initiatives_once() {
    let plan = planned().await;
    let goal = plan.goal("Ship the demo");
    assert!(
        kickoffs(&plan, &goal.id).is_empty(),
        "a goal in define waits"
    );

    let goal = activate(&plan, &goal);
    let card = plan
        .kickoff(&goal.id, "Propose initiatives for: ")
        .expect("a proposals kickoff");
    let read = item(&plan, &card);
    assert_eq!(read.assignee.as_deref(), Some(plan.lead.as_str()));
    assert_eq!(read.item_type, ItemType::Spike);
    assert_eq!(read.labels, ["kickoff"]);
    assert!(
        read.description.contains("plan_propose"),
        "{}",
        read.description
    );
    let notes = told(&plan.d.app.db, &plan.lead);
    assert!(
        notes.iter().any(|n| n.contains("Propose initiatives for")),
        "{notes:?}"
    );

    // Reading the plan again makes nothing twice.
    plan.owner().tree(&plan.pid).unwrap();
    plan.owner().tree(&plan.pid).unwrap();
    assert_eq!(kickoffs(&plan, &goal.id).len(), 1);

    // A goal met closes its proposals card.
    done(plan.owner().update(&c::PlanUpdate {
        id: goal.id.clone(),
        expected_version: goal.version,
        stage: Some(c::PlanStage::Met as i32),
        ..Default::default()
    }));
    assert!(closed(&plan, &card));
}

#[tokio::test]
async fn a_goal_active_before_its_lead_gets_the_card_on_the_next_read() {
    let plan = planned().await;
    let db = &plan.d.app.db;
    db.set_project_lead(&plan.pid, None).unwrap();
    db.remove_project_role(&plan.pid, Role::Lead, &plan.lead)
        .unwrap();
    let goal = activate(&plan, &plan.goal("Ship the demo"));
    assert!(plan.kickoff(&goal.id, "Propose ").is_none(), "no lead yet");

    plan.d
        .app
        .db
        .set_project_lead(&plan.pid, Some(&plan.lead))
        .unwrap();
    // The daemon settles every planned project when it starts, as a read
    // of the plan does.
    let planned = db.board_read(|t| t.planned_projects()).unwrap();
    assert_eq!(planned, std::slice::from_ref(&plan.pid));
    plan.owner().settle(&plan.pid).unwrap();
    let card = plan.kickoff(&goal.id, "Propose ").expect("the kickoff");
    assert_eq!(
        item(&plan, &card).assignee.as_deref(),
        Some(plan.lead.as_str())
    );
}

#[tokio::test]
async fn an_initiative_walks_the_lead_through_research_and_its_prd() {
    let plan = planned().await;
    let goal = plan.goal("Ship the demo");
    let init = plan.initiative(&goal.id, "Combat readability");

    // Research first, on the initiative's own board.
    let research = plan.kickoff(&init.id, "Research: ").expect("research");
    let read = item(&plan, &research);
    assert_eq!(Some(read.board_id.as_str()), init.board_id.as_deref());
    assert_eq!(read.item_type, ItemType::Spike);
    assert!(
        read.description
            .contains("docs/initiatives/combat-readability/research/"),
        "{}",
        read.description
    );

    // Skipped research closes it, and the PRD is next.
    let init = plan.skip(&init);
    assert!(closed(&plan, &research));
    let write = plan.kickoff(&init.id, "Write the PRD: ").expect("the PRD");
    let read = item(&plan, &write);
    assert_eq!(read.item_type, ItemType::Design);
    assert!(
        read.description
            .contains("docs/initiatives/combat-readability/prd.md"),
        "{}",
        read.description
    );

    // Submitted, it waits on the owner; sent back, a revision replaces it.
    let path = "docs/initiatives/combat/prd.md";
    let submitted = plan.submit(&init, path, &["Telegraphing"]);
    assert_eq!(
        kickoffs(&plan, &init.id).len(),
        2,
        "nothing new while it waits"
    );
    let back = done(plan.owner().rule(&c::PlanRule {
        id: submitted.id.clone(),
        expected_version: submitted.version,
        approve: false,
        note: Some("Say what the player sees".into()),
        ..Default::default()
    }));
    let revise = plan
        .kickoff(&init.id, "Revise the PRD: ")
        .expect("a revision");
    assert!(item(&plan, &revise)
        .description
        .contains("Say what the player sees"));
    assert!(closed(&plan, &write));
    assert!(!closed(&plan, &revise));

    // Approved, its card closes and the new epic's research starts.
    let again = plan.submit(&back, path, &["Telegraphing"]);
    let (init, created) = plan.approve(&again, &[]);
    assert_eq!(init.stage, Stage::Running);
    assert!(closed(&plan, &revise));
    let epic = &created[0];
    let card = plan
        .kickoff(epic, "Research: ")
        .expect("the epic's research");
    assert_eq!(
        item(&plan, &card).assignee.as_deref(),
        Some(plan.lead.as_str())
    );

    // Kickoffs are plan work, not the node's progress.
    let tree = plan.owner().tree(&plan.pid).unwrap();
    let read = tree.nodes.iter().find(|n| n.id == init.id).unwrap();
    assert_eq!((read.cards_total, read.cards_done), (0, 0));
}
