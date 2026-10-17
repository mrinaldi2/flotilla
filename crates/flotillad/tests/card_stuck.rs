//! What the board shows when a card can't move, and the owner's override
//! on a release-gated board (0.19.0 card flow); and a question about a
//! draft, which closes when the draft's discussion ends.

use crate::common;

use bus::contract::board::{self as c, board_request::Request, board_response::Response};
use common::board::{call, response};
use common::cards::{done, fresh};
use serde_json::json;

#[tokio::test]
async fn a_card_says_why_it_cant_move() {
    let (mut f, _) = fresh(&["Builder"]).await;
    let item = f.item("Product photos").await;

    // Nobody is assigned: the next column's rules stop it.
    let card = f.card(&item.id).await;
    let block = card.move_block.expect("a block");
    assert_eq!(
        (
            block.to_column.as_str(),
            block.reason.as_str(),
            block.refused
        ),
        ("doing", "Nobody is assigned.", false)
    );

    // A refused move shows instead, until it would pass or the card moves.
    let refused = f.owner_move(&item.id, "review", None).await;
    assert!(matches!(
        refused.outcome,
        Some(c::move_result::Outcome::Refused(_))
    ));
    let block = f.card(&item.id).await.move_block.expect("the refusal");
    assert_eq!((block.to_column.as_str(), block.refused), ("review", true));
    assert_eq!(block.reason, "Items come to Review only from doing.");

    // Assigned, it may start; in Doing, nothing stops it reaching Review.
    f.assign(&item.id, Some("Builder")).await;
    f.bots[0].call("item_get", json!({"id": item.id})).await;
    let card = f.card(&item.id).await;
    assert_eq!(card.column_key, "doing");
    assert_eq!(card.move_block, None, "{card:?}");

    // A bot's refused move shows on the card it pushes.
    let mut watcher = common::WsClient::connect(&f.d).await;
    call(
        &mut watcher,
        Request::BoardWatch(c::BoardWatch {
            project_id: f.project.clone(),
        }),
    )
    .await;
    f.bot_move(0, &item.id, "review").await;
    let own_done = f.bot_move(0, &item.id, "done").await;
    assert_eq!(own_done["isError"], json!(true));
    let mut last = None;
    while let Some(event) = common::board::next_push(&mut watcher).await {
        last = event.card.or(last);
    }
    let block = last
        .and_then(|card| card.move_block)
        .expect("the pushed refusal");
    assert_eq!((block.to_column.as_str(), block.refused), ("done", true));
    assert!(block.reason.contains("can't be its assignee"), "{block:?}");
}

#[tokio::test]
async fn a_release_gated_unlisted_move_is_the_owners_override() {
    let d = common::spawn_daemon().await;
    let mut owner = common::WsClient::connect(&d).await;
    let project = common::peers::project(&mut owner, "Old Shop").await;
    d.app
        .db
        .ensure_board(&project, &d.app.db.daemon_id().unwrap(), Some("OS"))
        .unwrap();
    let (id, _) = common::board::new_item(
        &d.app.db,
        &project,
        "Legacy",
        flotillad::kanban::model::Priority::P2,
    );
    let Response::MoveCheck(check) = response(
        call(
            &mut owner,
            Request::ItemMoveCheck(c::ItemMoveCheck { id: id.clone() }),
        )
        .await,
    ) else {
        panic!("expected a move check");
    };
    let flag = |key: &str| {
        check
            .columns
            .iter()
            .find(|col| col.column_key == key)
            .unwrap_or_else(|| panic!("no {key}"))
            .owner_override
    };
    // Inbox → Doing is in no row of the table: the owner's override.
    assert!(flag("doing"));
    assert!(flag("review"));
    // Ready (refine) and Cancelled (cancel) are rows of their own.
    assert!(!flag("ready"));
    assert!(!flag("cancelled"));
    // The card names the next row's rule, not the owner's way around it.
    let s = common::board::snapshot(
        call(
            &mut owner,
            Request::BoardGet(c::BoardGet {
                project_id: project.clone(),
            }),
        )
        .await,
    );
    let block = s
        .cards
        .iter()
        .find(|card| card.id == id)
        .and_then(|card| card.move_block.clone())
        .expect("a block");
    assert_eq!(block.to_column, "ready");
    assert!(!block.reason.is_empty() && !block.refused, "{block:?}");
}

#[tokio::test]
async fn a_question_about_a_draft_closes_when_its_discussion_ends() {
    let (mut f, _) = fresh(&["Builder"]).await;
    let created = call(
        &mut f.owner,
        Request::ItemCreate(c::ItemCreate {
            project_id: f.project.clone(),
            r#type: c::ItemType::Task as i32,
            title: "Gift cards".into(),
            draft: Some(true),
            ..Default::default()
        }),
    )
    .await;
    let draft = done(created);
    let invite = |remove: bool| {
        Request::ItemInvite(c::ItemInvite {
            id: draft.id.clone(),
            bot: Some("Builder".into()),
            remove: remove.then_some(true),
        })
    };
    let open = |f: &common::cards::Fresh| {
        flotillad::owner_threads::open_questions(&f.d.app, Some(&f.project), None).unwrap()
    };

    // The invited bot asks about the draft: no comment on it, a question
    // in its thread that names the draft.
    call(&mut f.owner, invite(false)).await;
    let asked = f.bots[0]
        .call(
            "message_owner",
            json!({"body": "Fixed amounts or any?", "asks": true, "item": draft.id}),
        )
        .await;
    assert_eq!(asked["asks"], true, "{asked}");
    let questions = open(&f);
    assert_eq!(questions.len(), 1, "{questions:?}");
    let tag = format!("[draft {} “Gift cards”]", draft.id);
    assert!(questions[0].title.contains(&tag), "{questions:?}");
    let comments =
        f.d.app
            .db
            .board_read(|t| t.item_comments(&draft.id))
            .unwrap();
    assert!(
        comments.is_empty(),
        "a draft takes no comment: {comments:?}"
    );
    // Ending the discussion closes it.
    call(&mut f.owner, invite(true)).await;
    assert!(open(&f).is_empty(), "{:?}", open(&f));

    // So does finalizing the draft.
    call(&mut f.owner, invite(false)).await;
    f.bots[0]
        .call(
            "message_owner",
            json!({"body": "Expiry date?", "asks": true, "item": draft.id}),
        )
        .await;
    assert_eq!(open(&f).len(), 1);
    let version = f.now(&draft.id).version;
    call(
        &mut f.owner,
        Request::ItemFinalize(c::ItemFinalize {
            id: draft.id.clone(),
            expected_version: version,
            column: None,
        }),
    )
    .await;
    assert_eq!(
        f.now(&draft.id).state,
        flotillad::kanban::model::ItemState::Live
    );
    assert!(open(&f).is_empty(), "{:?}", open(&f));
}
