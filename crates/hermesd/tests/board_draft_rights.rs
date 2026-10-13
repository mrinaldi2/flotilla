//! What a draft can't be used for (review of K4b): a bot holding the owner
//! token can't act as the owner on a draft, and a draft has no say over the
//! live board.

use crate::common;

use bus::contract::board::{self as c, board_request::Request, board_response::Response};
use common::board::*;
use common::tasks::project_with_bots;
use common::{TestDaemon, WsClient};
use serde_json::json;

fn project(d: &TestDaemon, ids: &[String]) -> String {
    let db = &d.app.db;
    let project_id = db.get_bot(&ids[0]).unwrap().unwrap().project_id;
    db.set_project_lead(&project_id, Some(&ids[0])).unwrap();
    db.ensure_board(&project_id, &db.daemon_id().unwrap(), Some("H"))
        .unwrap();
    project_id
}

async fn create(owner: &mut WsClient, project_id: &str, title: &str, draft: bool) -> c::Item {
    let reply = call(
        owner,
        Request::ItemCreate(c::ItemCreate {
            project_id: project_id.into(),
            r#type: c::ItemType::Bug as i32,
            title: title.into(),
            draft: Some(draft),
            ..Default::default()
        }),
    )
    .await;
    match response(reply) {
        Response::Edited(c::EditResult {
            outcome: Some(c::edit_result::Outcome::Done(item)),
            ..
        }) => item,
        other => panic!("expected an item, got {other:?}"),
    }
}

fn invite(id: &str, bot: &str) -> Request {
    Request::ItemInvite(c::ItemInvite {
        id: id.into(),
        bot: Some(bot.into()),
        remove: None,
    })
}

#[tokio::test]
async fn the_owner_token_a_bot_can_read_neither_finalizes_discards_invites_nor_marks_seen() {
    let (pair, mut bots) = project_with_bots(&["Lead", "Builder"]).await;
    let project_id = project(&pair.d, &pair.ids);
    let mut owner = WsClient::connect(&pair.d).await;
    let draft = create(&mut owner, &project_id, "Sort is wrong", true).await;
    call(&mut owner, invite(&draft.id, "Lead")).await;
    // The lead changes it: a Needs you row the owner must see.
    bots[0]
        .call(
            "item_update",
            json!({"id": draft.id, "expected_version": draft.version, "title": "Sort order"}),
        )
        .await;
    let version = draft.version + 1;

    let mut token = WsClient::connect_owner_token(&pair.d).await;
    let attempts = [
        invite(&draft.id, "Builder"),
        Request::ItemFinalize(c::ItemFinalize {
            id: draft.id.clone(),
            expected_version: version,
            column: None,
        }),
        Request::ItemDiscard(c::ItemDiscard {
            id: draft.id.clone(),
            expected_version: version,
            reason: "noise".into(),
        }),
        Request::ItemDraftChanges(c::ItemDraftChanges {
            id: draft.id.clone(),
            since: None,
            mark_seen: Some(true),
        }),
    ];
    for request in attempts {
        let code = error_code(call(&mut token, request).await);
        assert_eq!(code, "forbidden");
    }
    // Reading the changes without marking them seen is still a read.
    let read = call(
        &mut token,
        Request::ItemDraftChanges(c::ItemDraftChanges {
            id: draft.id.clone(),
            since: None,
            mark_seen: None,
        }),
    )
    .await;
    assert!(matches!(response(read), Response::DraftChanges(_)));
    let db = &pair.d.app.db;
    let item = db.get_item(&draft.id).unwrap().unwrap();
    assert_eq!(item.state.as_str(), "draft");
    assert_eq!(db.board_read(|t| t.invited(&draft.id)).unwrap().len(), 1);
    let unseen = db.board_read(|t| t.unseen_drafts(&project_id)).unwrap();
    assert_eq!(unseen.len(), 1, "the row stays until the owner looks");

    // The app, on its ticket, does all of it.
    let finalized = call(
        &mut owner,
        Request::ItemFinalize(c::ItemFinalize {
            id: draft.id.clone(),
            expected_version: version,
            column: None,
        }),
    )
    .await;
    assert!(matches!(moved(finalized), c::move_result::Outcome::Done(_)));
}

#[tokio::test]
async fn a_draft_blocks_no_live_item() {
    let (pair, mut bots) = project_with_bots(&["Lead", "Builder"]).await;
    let project_id = project(&pair.d, &pair.ids);
    let mut owner = WsClient::connect(&pair.d).await;
    let live = create(&mut owner, &project_id, "Crash on open", false).await;
    let draft = create(&mut owner, &project_id, "Maybe a redesign", true).await;
    // The builder, invited and holding no board role, links the draft as
    // blocking the live item.
    call(&mut owner, invite(&draft.id, "Builder")).await;
    bots[1]
        .call(
            "item_link",
            json!({"id": draft.id, "kind": "item:blocks", "ref": live.id}),
        )
        .await;
    let blockers = pair
        .d
        .app
        .db
        .board_read(|t| t.blockers_of(&live.id))
        .unwrap();
    assert!(blockers.is_empty(), "{blockers:?}");

    // Discarded, it never blocks either.
    call(
        &mut owner,
        Request::ItemDiscard(c::ItemDiscard {
            id: draft.id.clone(),
            expected_version: draft.version,
            reason: "not now".into(),
        }),
    )
    .await;
    let blockers = pair
        .d
        .app
        .db
        .board_read(|t| t.blockers_of(&live.id))
        .unwrap();
    assert!(blockers.is_empty(), "{blockers:?}");
}
