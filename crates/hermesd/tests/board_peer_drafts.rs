//! Drafts stay on the board's home (review of K4b): a linked computer reads
//! a live item's details from the home, never a draft's.

use crate::common;

use bus::contract::board::{self as c, board_request::Request};
use common::peer_board::board;
use hermesd::actor::Actor;
use hermesd::db::NewItem;
use hermesd::kanban::model::{ItemType, Priority};
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
                    platforms: &[],
                    size: None,
                    priority: Priority::P2,
                    labels: &[],
                    parent_id: None,
                    acceptance_criteria: &[],
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
