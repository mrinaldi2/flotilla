//! Linked computers on mixed versions (0.19.0 K6). A computer on 0.18 says
//! no kanban contract in its hello and sends no board ids: the home on
//! 0.19 gives it the project's default board as its one board. A computer
//! on 0.19 whose home is on 0.18 keeps the single board it is sent and asks
//! that home nothing it lacks. The tests stand an older computer in by
//! having it speak contract 1.

use crate::common;

use bus::contract::board::{self as c, board_request::Request, board_response::Response};
use common::board::{call, response};
use common::peer_board::{board_on, Board};
use common::peers::{computer, paired_from, wait_until};
use common::tasks::error_text;
use flotillad::actor::Actor;
use flotillad::kanban::admin::{self, By, Outcome, Structure};
use flotillad::kanban::presets::Preset;
use serde_json::json;

/// The board fixture with the PC (`older_pc`) or the Mac on contract 1.
async fn mixed(older_pc: bool) -> Board {
    let mac = computer(|_| {}).await;
    let win = computer(|_| {}).await;
    let older = if older_pc { &win } else { &mac };
    older.app.peers.speak_kanban_for_tests(1);
    board_on(paired_from(mac, win).await).await
}

async fn create(b: &mut Board, board_id: Option<String>, title: &str) -> String {
    let created = call(
        &mut b.p.mac_client,
        Request::ItemCreate(c::ItemCreate {
            project_id: b.mac_app.clone(),
            r#type: c::ItemType::Feature as i32,
            title: title.into(),
            board_id,
            ..Default::default()
        }),
    )
    .await;
    match response(created) {
        Response::Edited(c::EditResult {
            outcome: Some(c::edit_result::Outcome::Done(item)),
            ..
        }) => item.id,
        other => panic!("expected the item, got {other:?}"),
    }
}

#[tokio::test]
async fn a_0_18_computer_is_served_the_default_board_alone() {
    let mut b = mixed(true).await;
    let mac = b.p.mac.app.clone();
    assert_eq!(
        mac.peers
            .kanban(&b.p.mac.app.db.list_peers().unwrap()[0].id),
        1
    );
    let Outcome::Applied { board_id: rs } = admin::structure(
        &mac,
        By::Owner(Actor::User),
        Structure::Create(c::BoardCreate {
            project_id: b.mac_app.clone(),
            name: "Research".into(),
            key: Some("RS".into()),
            preset: Preset::Simple.name().into(),
            ..Default::default()
        }),
    )
    .expect("created") else {
        panic!("the owner's change applies");
    };
    let elsewhere = create(&mut b, Some(rs.clone()), "On another board").await;
    let main = create(&mut b, None, "On the default board").await;

    // Its mirror holds the default board's columns and cards, and hears of
    // no other board.
    let (win, win_app) = (b.p.win.app.clone(), b.win_app.clone());
    wait_until("the PC mirrors the default board's new card", || {
        win.board_mirror
            .get(&win_app)
            .is_some_and(|m| m.snapshot.cards.iter().any(|card| card.id == main))
    })
    .await;
    let held = win.board_mirror.get(&win_app).expect("mirrored").snapshot;
    assert!(held.boards.is_empty(), "{:?}", held.boards);
    assert!(held.cards.iter().all(|card| card.id != elsewhere));
    assert!(held.columns.iter().all(|col| col.board_id != rs));

    // Its frames name no board, and a board id it sends is not read.
    for frame in [
        json!({"type": "board_snapshot", "project_id": win_app}),
        json!({"type": "board_snapshot", "project_id": win_app, "board_id": rs}),
    ] {
        let reply = win.peers.request(&b.p.win_peer_id, frame).await.unwrap();
        assert!(reply["board_id"].is_null(), "{reply}");
        let snapshot: c::BoardSnapshot = serde_json::from_value(reply["board"].clone()).unwrap();
        assert!(snapshot.boards.is_empty());
        assert!(snapshot.cards.iter().any(|card| card.id == main));
        assert!(snapshot.cards.iter().all(|card| card.id != elsewhere));
    }
    let frame = json!({"type": "board_call", "project_id": win_app, "bot_id": b.tester_id,
                       "tool": "board_get", "args": {}});
    let got = win.peers.request(&b.p.win_peer_id, frame).await.unwrap();
    let cards = got["cards"].as_array().expect("cards");
    assert!(cards.iter().any(|card| card["id"] == main), "{got}");
    assert!(cards.iter().all(|card| card["id"] != elsewhere), "{got}");

    // What its bot files lands on the default board.
    let filed = b
        .tester
        .call("item_create", json!({"type": "bug", "title": "From win"}))
        .await;
    let id = filed["item"]["id"].as_str().expect("id");
    let default = mac.db.default_board(&b.mac_app).unwrap().expect("default");
    assert_eq!(mac.db.get_item(id).unwrap().unwrap().board_id, default.id);
}

#[tokio::test]
async fn a_0_19_computer_falls_back_to_one_board_with_a_0_18_home() {
    let mut b = mixed(false).await;
    let held = b.p.win.app.board_mirror.get(&b.win_app).expect("mirrored");
    assert!(held.snapshot.boards.is_empty());

    // Its board list is the one board it holds.
    let listed = b.tester.call("board_list", json!({})).await;
    let boards = listed["boards"].as_array().expect("boards");
    assert_eq!(boards.len(), 1, "{listed}");
    assert_eq!(boards[0]["board"]["key"], "H");
    assert!(!boards[0]["columns"].as_array().unwrap().is_empty());

    // What the home lacks is refused here, before it is asked.
    for (tool, args) in [
        (
            "column_add",
            json!({"board_id": "b", "version": 1, "name": "QA"}),
        ),
        (
            "item_attach",
            json!({"id": b.item, "url": "https://example.com"}),
        ),
    ] {
        let raw = b.tester.call_raw(tool, args).await;
        assert!(
            error_text(&raw).contains("runs an older Flotilla release"),
            "{raw}"
        );
    }

    // A board id is left out of what it asks: there is one board.
    let filed = b
        .tester
        .call(
            "item_create",
            json!({"type": "bug", "title": "From win", "board_id": "no-such-board"}),
        )
        .await;
    let id = filed["item"]["id"].as_str().expect("id");
    assert!(b.p.mac.app.db.get_item(id).unwrap().is_some(), "{filed}");

    // Its platforms go as a 0.18 home reads them, and come back as labels.
    let filed = b
        .tester
        .call(
            "item_create",
            json!({"type": "bug", "title": "On iOS", "labels": ["urgent", "platform:ios"]}),
        )
        .await;
    let id = filed["item"]["id"].as_str().expect("id");
    let item = b.p.mac.app.db.get_item(id).unwrap().expect("filed");
    assert_eq!(item.labels, ["urgent", "platform:ios"], "{filed}");
    // A 0.18 home filters by platform only.
    let raw = b
        .tester
        .call_raw("item_query", json!({"labels": ["urgent"]}))
        .await;
    assert!(
        error_text(&raw).contains("filters by platform only"),
        "{raw}"
    );
}
