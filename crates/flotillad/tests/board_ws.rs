//! The board over WebSocket binary frames (H-020 §1.5, B4): typed requests
//! in protobuf envelopes, grants, guarded moves, and `board_event` pushes to
//! the connections watching a project.

use crate::common;

use std::time::Duration;

use bus::contract::board::{
    self as c, board_request::Request, board_response::Response, move_result::Outcome,
};
use common::board::*;
use common::*;
use flotillad::actor::Actor;
use flotillad::db::NewItem;
use flotillad::kanban::model::{ItemType, Priority};
use serde_json::json;

#[tokio::test]
async fn the_board_is_read_moved_and_watched_over_binary_frames() {
    let d = spawn_daemon().await;
    let mut owner = WsClient::connect(&d).await;
    let mut watcher = WsClient::connect(&d).await;
    let project_id = common::peers::project(&mut owner, "Flotilla").await;

    // The first board_get with the control grant enables the board here.
    let board = snapshot(
        call(
            &mut owner,
            Request::BoardGet(c::BoardGet {
                project_id: project_id.clone(),
            }),
        )
        .await,
    );
    let settings = board.settings.expect("settings");
    assert_eq!(settings.home_daemon_id, d.app.db.daemon_id().expect("id"));
    // A new project's first board is Simple + Review (0.19.0).
    let names: Vec<&str> = board.columns.iter().map(|col| col.name.as_str()).collect();
    assert_eq!(names, ["To do", "Doing", "Review", "Done", "Cancelled"]);
    assert_eq!(board.boards[0].flow, "custom");
    assert!(board.cards.is_empty());
    assert_eq!(board.seq, 0);

    let item = d
        .app
        .db
        .create_item(
            &NewItem {
                project_id: &project_id,
                board_id: None,
                item_type: ItemType::Task,
                title: "Board over WS",
                description: "",
                size: None,
                priority: Priority::P1,
                labels: &["platform:daemon".to_string()],
                parent_id: None,
                acceptance_criteria: &[],
            },
            &Actor::User,
        )
        .expect("item");

    // Watching answers with the snapshot the pushes continue from.
    let watched = snapshot(
        call(
            &mut watcher,
            Request::BoardWatch(c::BoardWatch {
                project_id: project_id.clone(),
            }),
        )
        .await,
    );
    assert_eq!(watched.seq, 0);
    assert_eq!(watched.cards.len(), 1);
    assert_eq!(watched.cards[0].id, item.id);

    // A move the column's rules refuse writes nothing; its card is pushed
    // with why it can't move.
    let Outcome::Refused(refused) =
        moved(call(&mut owner, move_to(&item.id, "review", item.version, None)).await)
    else {
        panic!("Review takes items only from Doing");
    };
    assert!(
        refused.unmet.iter().any(|u| u.code == "move.from"),
        "{refused:?}"
    );
    let event = next_push(&mut watcher).await.expect("the refused card");
    assert_eq!(
        (event.seq, event.kind),
        (1, c::BoardEventKind::ItemUpserted as i32)
    );
    let block = event
        .card
        .and_then(|card| card.move_block)
        .expect("a block");
    assert_eq!((block.to_column.as_str(), block.refused), ("review", true));
    assert_eq!(block.reason, "Items come to Review only from doing.");

    // A stale version is a conflict, with the item as it is.
    let Outcome::Conflict(current) =
        moved(call(&mut owner, move_to(&item.id, "cancelled", 99, Some("dup"))).await)
    else {
        panic!("a stale version is a conflict");
    };
    assert_eq!(current.version, item.version);

    // What each column needs, for this connection.
    let Response::MoveCheck(check) = response(
        call(
            &mut owner,
            Request::ItemMoveCheck(c::ItemMoveCheck {
                id: item.id.clone(),
            }),
        )
        .await,
    ) else {
        panic!("expected a move check");
    };
    assert_eq!(check.columns.len(), 4, "every column but its own");
    assert!(
        check.columns.iter().all(|col| !col.owner_override),
        "a custom board lists every move it takes"
    );
    let cancel = check
        .columns
        .iter()
        .find(|col| col.column_key == "cancelled")
        .expect("cancelled");
    assert!(cancel.unmet.iter().any(|u| u.code == "reason.required"));

    // A move that holds commits, and the watcher hears it under seq 2.
    let Outcome::Done(done) = moved(
        call(
            &mut owner,
            move_to(
                &item.id,
                "cancelled",
                item.version,
                Some("Folded into H-020"),
            ),
        )
        .await,
    ) else {
        panic!("the owner may cancel with a reason");
    };
    assert_eq!(done.column_key, "cancelled");
    let event = next_push(&mut watcher).await.expect("a push");
    assert_eq!(event.seq, 2);
    assert_eq!(event.kind, c::BoardEventKind::ItemMoved as i32);
    assert_eq!(event.item_id, item.id);
    assert_eq!(event.from_column.as_deref(), Some("todo"));
    let card = event.card.expect("card");
    assert_eq!(
        (card.column_key.as_str(), card.version),
        ("cancelled", done.version)
    );

    // The drawer: the item, its links, comments and history.
    let Response::Item(detail) = response(
        call(
            &mut owner,
            Request::ItemGet(c::ItemGet {
                id: item.id.clone(),
            }),
        )
        .await,
    ) else {
        panic!("expected an item");
    };
    assert_eq!(detail.item.expect("item").version, done.version);
    let kinds: Vec<i32> = detail.history.iter().map(|e| e.kind).collect();
    assert_eq!(
        kinds,
        [
            c::ItemEventKind::Created as i32,
            c::ItemEventKind::Moved as i32
        ]
    );
    assert_eq!(detail.history[1].note.as_deref(), Some("Folded into H-020"));
    assert_eq!(detail.history_next, None);

    // History pages.
    let page = |after| {
        Request::ItemHistory(c::ItemHistory {
            id: item.id.clone(),
            after,
            limit: 1,
        })
    };
    let Response::History(first) = response(call(&mut owner, page(None)).await) else {
        panic!("expected history");
    };
    assert_eq!(first.events.len(), 1);
    let Response::History(second) = response(call(&mut owner, page(first.next)).await) else {
        panic!("expected history");
    };
    assert_eq!(second.events.len(), 1);
    assert_eq!(second.next, None);
    assert_ne!(first.events[0].id, second.events[0].id);

    // Queries filter cards.
    let query = |columns: &[&str]| {
        Request::ItemQuery(c::ItemQuery {
            project_id: project_id.clone(),
            column_keys: columns.iter().map(|s| (*s).to_string()).collect(),
            ..Default::default()
        })
    };
    let Response::Items(hit) = response(call(&mut owner, query(&["cancelled"])).await) else {
        panic!("expected items");
    };
    assert_eq!(hit.cards.len(), 1);
    let Response::Items(miss) = response(call(&mut owner, query(&["todo"])).await) else {
        panic!("expected items");
    };
    assert!(miss.cards.is_empty());
    let Response::Items(found) = response(
        call(
            &mut owner,
            Request::ItemQuery(c::ItemQuery {
                project_id: project_id.clone(),
                text: Some("WS".to_string()),
                ..Default::default()
            }),
        )
        .await,
    ) else {
        panic!("expected items");
    };
    assert_eq!(found.cards.len(), 1);

    // A later snapshot is current with the pushes so far.
    let again = snapshot(
        call(
            &mut owner,
            Request::BoardGet(c::BoardGet {
                project_id: project_id.clone(),
            }),
        )
        .await,
    );
    assert_eq!(again.seq, 2);

    // After unwatching, the next change is not pushed.
    let Response::Unwatched(_) = response(
        call(
            &mut watcher,
            Request::BoardUnwatch(c::BoardUnwatch {
                project_id: project_id.clone(),
            }),
        )
        .await,
    ) else {
        panic!("expected unwatched");
    };
    let Outcome::Done(_) = moved(
        call(
            &mut owner,
            move_to(&item.id, "todo", done.version, Some("Back to To do")),
        )
        .await,
    ) else {
        panic!("the owner may bring a cancelled item back");
    };
    assert!(next_push(&mut watcher).await.is_none());
}

#[tokio::test]
async fn grants_and_bad_requests_are_refused() {
    let d = spawn_daemon().await;
    let mut owner = WsClient::connect(&d).await;
    let project_id = common::peers::project(&mut owner, "Reader Test").await;
    let created = owner
        .request(json!({"type": "create_device", "name": "phone", "capabilities": ["read"]}))
        .await;
    let mut reader = connect_with(&d, token_str(&created)).await;

    // Reading can't enable a board; the owner's first read does.
    let get = || {
        Request::BoardGet(c::BoardGet {
            project_id: project_id.clone(),
        })
    };
    assert_eq!(error_code(call(&mut reader, get()).await), "no_board");
    snapshot(call(&mut owner, get()).await);
    let board = snapshot(call(&mut reader, get()).await);
    assert_eq!(board.columns.len(), 5);

    // Moving needs control.
    assert_eq!(
        error_code(call(&mut reader, move_to("TR-001", "ready", 1, None)).await),
        "forbidden"
    );

    // Unknown things are not found.
    assert_eq!(
        error_code(
            call(
                &mut owner,
                Request::ItemGet(c::ItemGet {
                    id: "TR-999".to_string()
                })
            )
            .await
        ),
        "not_found"
    );
    assert_eq!(
        error_code(call(&mut owner, move_to("TR-999", "ready", 1, None)).await),
        "not_found"
    );
    assert_eq!(
        error_code(
            call(
                &mut owner,
                Request::BoardGet(c::BoardGet {
                    project_id: "nope".to_string()
                })
            )
            .await
        ),
        "not_found"
    );

    // An unknown enum number in a filter is refused, not guessed.
    assert_eq!(
        error_code(
            call(
                &mut owner,
                Request::ItemQuery(c::ItemQuery {
                    project_id: project_id.clone(),
                    types: vec![99],
                    ..Default::default()
                })
            )
            .await
        ),
        "invalid_request"
    );

    // Garbage is answered under req_id 0, and the JSON protocol still works
    // on the same socket.
    send_raw(&mut owner, vec![0xff, 0xff]).await;
    let envelope = next_envelope(&mut owner, Duration::from_secs(5))
        .await
        .expect("a reply");
    assert_eq!(envelope.req_id, 0);
    assert_eq!(error_code(envelope.body.expect("body")), "invalid_request");
    let projects = owner.request(json!({"type": "list_projects"})).await;
    assert_eq!(projects["type"], "projects", "{projects}");
}
