//! Nodes closing as their work ships (0.21.0, #43): a feature is released
//! once its cards are done (a cancelled one aside), and its epic and
//! initiative close after it, on the next read or as soon as a card moves.
//! Turning planning on rewrites the bots' prompts and tells them.

use crate::common;

use bus::contract::board::{self as c, board_request::Request};
use common::board::call;
use common::plan::*;
use flotillad::kanban::model::ColumnCategory;
use flotillad::plan::model::{Node, Stage};

/// A goal's whole chain down to a feature in build with `tasks` cards.
fn to_build(plan: &Plan, tasks: &[&str]) -> (Node, Node, Node, Vec<String>) {
    let goal = plan.goal("Ship the demo");
    let init = plan.skip(&plan.initiative(&goal.id, "Combat readability"));
    let init = plan.submit(&init, "docs/initiatives/combat/prd.md", &["Telegraphing"]);
    let (init, epics) = plan.approve(&init, &[]);
    let epic = plan.skip(&plan.node(&epics[0]));
    let epic = plan.submit(&epic, "docs/initiatives/combat/epics/t/design.md", &[]);
    let (epic, _) = plan.approve(&epic, &[]);
    let epic = plan.submit(&epic, "docs/initiatives/combat/epics/t/hld.md", &["Cue"]);
    let (epic, features) = plan.approve(&epic, &[]);
    let feature = plan.node(&features[0]);
    let feature = plan.submit(&feature, "docs/initiatives/combat/epics/t/lld.md", tasks);
    let (feature, cards) = plan.approve(&feature, &[]);
    assert_eq!(feature.stage, Stage::Build);
    (init, epic, feature, cards)
}

fn column(plan: &Plan, board: &str, category: ColumnCategory) -> String {
    plan.d
        .app
        .db
        .board_columns(&plan.pid)
        .unwrap()
        .into_iter()
        .find(|c| c.board_id == board && c.category == category)
        .expect("a column")
        .key
}

#[tokio::test]
async fn a_feature_is_released_and_its_epic_and_initiative_close_on_read() {
    let plan = planned().await;
    let (init, epic, feature, cards) = to_build(&plan, &["A", "B", "C"]);
    plan.finish(&cards[0]);
    plan.owner().tree(&plan.pid).unwrap();
    assert_eq!(
        plan.node(&feature.id).stage,
        Stage::Build,
        "two cards to go"
    );

    // A cancelled card doesn't hold the feature back.
    let board = init.board_id.clone().expect("a board");
    let cancelled = column(&plan, &board, ColumnCategory::Cancelled);
    let item = plan.d.app.db.get_item(&cards[2]).unwrap().unwrap();
    let to = flotillad::db::MoveTo {
        column: &cancelled,
        ..Default::default()
    };
    plan.d
        .app
        .db
        .move_item(&cards[2], item.version, &to, &flotillad::actor::Actor::User)
        .unwrap();
    plan.finish(&cards[1]);
    plan.owner().tree(&plan.pid).unwrap();
    assert_eq!(plan.node(&feature.id).stage, Stage::Released);
    assert_eq!(plan.node(&epic.id).stage, Stage::Done);
    assert_eq!(plan.node(&init.id).stage, Stage::Done);
    let kinds: Vec<String> = plan
        .owner()
        .detail(&feature.id)
        .unwrap()
        .unwrap()
        .history
        .into_iter()
        .map(|e| e.kind)
        .collect();
    assert_eq!(kinds.last().map(String::as_str), Some("released"));
}

#[tokio::test]
async fn a_card_moved_to_done_releases_its_feature_without_a_read() {
    let mut plan = planned().await;
    let (init, _, feature, cards) = to_build(&plan, &["Only"]);
    let board = init.board_id.clone().expect("a board");
    let done = column(&plan, &board, ColumnCategory::Done);
    let item = plan.d.app.db.get_item(&cards[0]).unwrap().unwrap();
    call(
        &mut plan.owner,
        Request::ItemMove(c::ItemMove {
            id: cards[0].clone(),
            to: done,
            expected_version: item.version,
            ..Default::default()
        }),
    )
    .await;
    let db = &plan.d.app.db;
    common::peers::wait_until("the feature is released", || {
        db.board_read(|t| t.plan_node(&feature.id))
            .ok()
            .flatten()
            .is_some_and(|n| n.stage == Stage::Released)
    })
    .await;
}

#[tokio::test]
async fn turning_planning_on_rewrites_the_bots_prompts_and_tells_them() {
    let plan = unplanned().await;
    let bot = plan.d.app.db.get_bot(&plan.lead).unwrap().unwrap();
    let project = plan.d.app.db.get_project(&plan.pid).unwrap().unwrap();
    let md = flotillad::paths::bot_dir(&plan.d.app.cfg, &project.dir_name, &bot.dir_name)
        .join("system.md");
    let before = std::fs::read_to_string(&md).unwrap_or_default();
    assert!(!before.contains("## The plan"));
    assert!(plan.owner().enable(&plan.pid, true).unwrap().is_none());
    let after = std::fs::read_to_string(&md).expect("system.md");
    assert!(after.contains("## The plan"), "{after}");
    let conversation = plan
        .d
        .app
        .db
        .dm_conversation(&plan.lead)
        .unwrap()
        .expect("a conversation");
    let told = plan
        .d
        .app
        .db
        .list_messages(&conversation.id, None, 50)
        .unwrap()
        .iter()
        .any(|m| m.body.contains("turned on planning"));
    assert!(told, "the lead was told");
}
