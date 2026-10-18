//! Drafts stay on the board's home (review of K4b): a linked computer reads
//! a live item's details from the home, never a draft's, and a linked bot
//! discusses one only while it is invited.

use crate::common;

use bus::contract::board::{self as c, board_request::Request};
use common::board::{call, response, version};
use common::peer_board::board;
use common::peer_files::{owner_attach, screenshot};
use common::peers::wait_until;
use common::tasks::error_text;
use common::terminal;
use flotillad::actor::Actor;
use flotillad::db::NewItem;
use flotillad::kanban::model::{ItemType, Priority};
use serde_json::json;

fn read(project_id: &str, id: &str) -> serde_json::Value {
    let request = c::BoardRequest {
        request: Some(Request::ItemGet(c::ItemGet { id: id.into() })),
    };
    json!({"type": "board_read", "project_id": project_id,
           "request": serde_json::to_value(request).unwrap()})
}

#[tokio::test]
async fn a_linked_computer_reads_no_draft_from_the_home() {
    let b = board().await;
    let db = &b.p.mac.app.db;
    let draft = db
        .board_tx(|t| {
            t.create_draft(
                &NewItem {
                    project_id: &b.mac_app,
                    board_id: None,
                    item_type: ItemType::Bug,
                    title: "Not on the board yet",
                    description: "",
                    size: None,
                    priority: Priority::P2,
                    labels: &[],
                    parent_id: None,
                    acceptance_criteria: &[],
                    plan_node_id: None,
                },
                &Actor::User,
            )
        })
        .unwrap();
    let peers = &b.p.win.app.peers;
    let live = peers
        .request(&b.p.win_peer_id, read(&b.win_app, &b.item))
        .await;
    assert!(live.is_ok(), "{live:?}");
    let refused = peers
        .request(&b.p.win_peer_id, read(&b.win_app, &draft.id))
        .await
        .expect_err("a draft isn't served");
    assert!(format!("{refused:?}").contains("no item"), "{refused:?}");
}

/// A bot on the linked computer, invited to a draft, is sent it with its
/// files named but kept on the home, edits it through the home, and once
/// uninvited is told so and refused it.
#[tokio::test]
async fn a_linked_bot_discusses_a_draft_until_it_is_uninvited() {
    let mut b = board().await;
    let owner = &mut b.p.mac_client;
    let created = call(
        owner,
        Request::ItemCreate(c::ItemCreate {
            project_id: b.mac_app.clone(),
            r#type: c::ItemType::Bug as i32,
            title: "Crash on open".into(),
            draft: Some(true),
            ..Default::default()
        }),
    )
    .await;
    let c::board_response::Response::Edited(c::EditResult {
        outcome: Some(c::edit_result::Outcome::Done(draft)),
        ..
    }) = response(created)
    else {
        panic!("expected the draft");
    };
    let id = draft.id;
    owner_attach(owner, &id, "crash.png", &screenshot(2048)).await;
    let invite = |remove: bool| {
        Request::ItemInvite(c::ItemInvite {
            id: id.clone(),
            bot: Some(b.stand_in.clone()),
            remove: remove.then_some(true),
        })
    };
    call(owner, invite(false)).await;
    let (win, tester_id) = (&b.p.win, b.tester_id.clone());
    wait_until("the kickoff reaches the PC's session", || {
        terminal(win, &tester_id).contains("The owner wants to discuss draft")
    })
    .await;
    let term = terminal(win, &tester_id);
    assert!(
        term.contains("- [image] crash.png: kept on the board's computer"),
        "{term}"
    );
    let mac_artifacts = b.p.mac.app.cfg.home.display().to_string();
    assert!(!term.contains(&mac_artifacts), "no home path: {term}");
    let got = b.tester.call("item_get", json!({ "id": id })).await;
    assert_eq!(got["item"]["state"], "draft", "{got}");

    call(&mut b.p.mac_client, invite(true)).await;
    wait_until("the PC's session hears the discussion ended", || {
        terminal(win, &tester_id).contains("The owner ended the discussion of draft")
    })
    .await;
    for (tool, args) in [
        ("item_get", json!({ "id": id })),
        (
            "item_update",
            json!({"id": id, "expected_version": version(&got["item"]), "title": "x"}),
        ),
    ] {
        let raw = b.tester.call_raw(tool, args).await;
        assert!(error_text(&raw).contains("no item"), "{tool}: {raw}");
    }
}
