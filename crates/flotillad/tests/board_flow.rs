//! The board runs the SDLC (0.24.0 step 2, `board_flow_v1`): the first
//! board's columns assign Product, Architect, Developer and Tester on entry,
//! each role moves the card on, the Tester closes a passed card or sends a
//! failed one back to its Developer with the reason, and the owner's moves
//! and a fact's assign as a bot's do.

use crate::common;

use bus::contract::board::{self as c, board_request::Request};
use bus::BotState;
use common::board::{call, CODE_RESEARCH};
use common::cards::{done, fresh, Fresh};
use common::peers::wait_until;
use flotillad::actor::Actor;
use flotillad::kanban::api::{publish_moves, report_fact};
use flotillad::kanban::facts::Fact;
use flotillad::kanban::model::{ProjectRole, Role};
use flotillad::kanban::presets::Preset;
use serde_json::{json, Value};

/// A Simple + Review project whose first bot leads and whose other bots
/// hold `roles`, in order.
async fn team(names: &[&str], roles: &[Role]) -> Fresh {
    let (f, _) = fresh(names).await;
    for (i, role) in roles.iter().enumerate() {
        f.d.app
            .db
            .set_project_role(&ProjectRole {
                project_id: f.project.clone(),
                role: *role,
                bot_id: f.bot_ids[i + 1].clone(),
                machine: None,
            })
            .unwrap();
    }
    f
}

fn assignee(f: &Fresh, id: &str) -> Option<String> {
    f.now(id).assignee
}

/// Bot `i` moves the card over MCP, with a reason when given.
async fn bot_move(f: &mut Fresh, i: usize, id: &str, to: &str, reason: Option<&str>) -> Value {
    let version = f.now(id).version;
    let mut args = json!({"id": id, "to": to, "expected_version": version});
    if let Some(reason) = reason {
        args["reason"] = json!(reason);
    }
    let moved = f.bots[i].call_raw("item_move", args).await;
    assert_ne!(moved["isError"], json!(true), "{moved}");
    moved
}

#[tokio::test]
async fn a_card_runs_the_chain_from_to_do_to_done() {
    use Role::*;
    let names = ["Lead", "Pat", "Ari", "Dev", "Tess"];
    let mut f = team(&names, &[Product, Architect, Dev, Tester]).await;
    let [_, pat, ari, dev, tess] = f.bot_ids.clone().try_into().unwrap();

    // The first board starts with the default chain, and its Done lets the
    // Tester close a card.
    let s = f.board().await;
    let on_entry: Vec<(String, i32)> = s
        .columns
        .iter()
        .map(|col| (col.key.clone(), col.on_entry))
        .collect();
    let wire = |r: c::Role| r as i32;
    assert_eq!(
        on_entry[..5],
        [
            ("todo".to_string(), wire(c::Role::Product)),
            ("research".to_string(), wire(c::Role::Architect)),
            ("doing".to_string(), wire(c::Role::Dev)),
            ("review".to_string(), wire(c::Role::Tester)),
            ("done".to_string(), wire(c::Role::Unspecified)),
        ]
    );
    let done_col = s.columns.iter().find(|col| col.key == "done").unwrap();
    assert!(done_col.movers.contains(&"roles:tester".to_string()));

    // A new card enters To do: the Product gets it and shapes it there.
    let item = f.item("Export orders").await;
    assert_eq!(assignee(&f, &item.id).as_deref(), Some(pat.as_str()));
    let told = f.heard(1, "where the Product takes it").await;
    assert!(told.contains("write its acceptance"), "{told}");
    assert!(
        told.contains("Then move it to Research with item_move."),
        "{told}"
    );
    let read = f.bots[1].call("item_get", json!({"id": item.id})).await;
    assert_eq!(read["item"]["column_key"], "todo", "{read}");

    // Each role moves it on, and the next column's role gets it.
    bot_move(&mut f, 1, &item.id, "research", None).await;
    assert_eq!(assignee(&f, &item.id).as_deref(), Some(ari.as_str()));
    let told = f.heard(2, "where the Architect takes it").await;
    assert!(told.contains("Pat moved"), "{told}");
    f.research(2, &item.id).await;
    bot_move(&mut f, 2, &item.id, "doing", None).await;
    assert_eq!(assignee(&f, &item.id).as_deref(), Some(dev.as_str()));
    f.heard(3, "where the Developer takes it").await;
    bot_move(&mut f, 3, &item.id, "review", None).await;
    assert_eq!(assignee(&f, &item.id).as_deref(), Some(tess.as_str()));
    let told = f.heard(4, "where the Tester takes it").await;
    assert!(told.contains("If it passes, move it to Done."), "{told}");

    // The Tester, which only checked it, closes it.
    bot_move(&mut f, 4, &item.id, "done", None).await;
    assert_eq!(f.now(&item.id).column_key, "done");
    let history = f.d.app.db.item_events(&item.id).unwrap();
    let routed: Vec<(&str, Option<&str>)> = history
        .iter()
        .filter(|e| e.field.as_deref() == Some("on_entry"))
        .map(|e| (e.actor.as_str(), e.note.as_deref()))
        .collect();
    assert_eq!(
        routed,
        [
            ("daemon", Some("product")),
            ("daemon", Some("architect")),
            ("daemon", Some("dev")),
            ("daemon", Some("tester")),
        ]
    );
}

#[tokio::test]
async fn a_failed_check_goes_back_to_its_developer_with_the_reason() {
    use Role::*;
    let names = ["Lead", "Ari", "Dev", "Dax", "Tess"];
    let mut f = team(&names, &[Architect, Dev, Dev, Tester]).await;
    for id in &f.bot_ids[2..4] {
        let (app, id) = (f.d.app.clone(), id.clone());
        wait_until("the dev is up", move || {
            app.supervisor.state(&id).0 == BotState::Ready
        })
        .await;
    }
    let item = f.item("Import CSV").await;
    f.assign(&item.id, Some("Ari")).await;
    bot_move(&mut f, 1, &item.id, "research", None).await;
    f.research(1, &item.id).await;
    bot_move(&mut f, 1, &item.id, "doing", None).await;
    let builder = assignee(&f, &item.id).expect("a developer");
    let i = f.bot_ids.iter().position(|b| *b == builder).unwrap();
    assert!(i == 2 || i == 3, "{builder}");
    bot_move(&mut f, i, &item.id, "review", None).await;
    assert_eq!(assignee(&f, &item.id), Some(f.bot_ids[4].clone()));

    // The builder is busy now and the other Developer free: the card still
    // goes back to the one that built it, with the Tester's reason.
    f.d.app
        .supervisor
        .set_state(&builder, BotState::Working, "");
    let reason = "The export has no header row";
    bot_move(&mut f, 4, &item.id, "doing", Some(reason)).await;
    assert_eq!(assignee(&f, &item.id).as_deref(), Some(builder.as_str()));
    let comments = f.d.app.db.board_read(|t| t.item_comments(&item.id)).unwrap();
    let posted = comments.last().expect("the reason");
    assert_eq!(posted.body, format!("Sent back to Doing: {reason}"));
    assert_eq!(posted.author, format!("bot:{}", f.bot_ids[4]));
    let told = f.heard(i, "It was sent back").await;
    assert!(told.contains(&format!("“{reason}”")), "{told}");
    assert!(told.contains("Tess moved"), "{told}");
}

#[tokio::test]
async fn the_owners_move_and_a_facts_move_assign_too() {
    use Role::*;
    let mut f = team(&["Lead", "Pat", "Dev", "Tess"], &[Product, Dev, Tester]).await;
    let board =
        f.d.app
            .db
            .create_board(&f.project, "Ship", "SW", Preset::Software)
            .unwrap();
    let req = Request::ItemCreate(c::ItemCreate {
        project_id: f.project.clone(),
        board_id: Some(board.id.clone()),
        r#type: c::ItemType::Task as i32,
        title: "Ship the export".into(),
        description: Some(CODE_RESEARCH.into()),
        ..Default::default()
    });
    let item = done(call(&mut f.owner, req).await);
    assert_eq!(item.column_key, "inbox");
    assert_eq!(assignee(&f, &item.id), Some(f.bot_ids[1].clone()));

    // The owner moves it to Doing: the Developer gets it.
    f.owner_move(&item.id, "sw-doing", None).await;
    assert_eq!(f.now(&item.id).column_key, "sw-doing");
    assert_eq!(assignee(&f, &item.id), Some(f.bot_ids[2].clone()));
    f.heard(2, "The owner moved").await;

    // Its PR opens: the fact moves it to Review, where the Tester gets it.
    let app = f.d.app.clone();
    let project = f.project.clone();
    let from = app
        .db
        .board_tx(|t| {
            report_fact(
                t,
                &project,
                &item.id,
                Fact::PrOpened,
                "pr.opened: #4",
                false,
                &Actor::Daemon,
            )
        })
        .unwrap()
        .expect("moved");
    publish_moves(
        &app,
        &mut app.board.writer(),
        &project,
        &[(item.id.clone(), from)],
    );
    assert_eq!(f.now(&item.id).column_key, "sw-review");
    assert_eq!(assignee(&f, &item.id), Some(f.bot_ids[3].clone()));
    let told = f.heard(3, "moved to Review, where the Tester").await;
    assert!(told.starts_with(&format!("[card {}]", item.id)), "{told}");
}
