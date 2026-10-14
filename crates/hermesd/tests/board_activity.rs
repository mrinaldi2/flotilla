//! Who did what on a board, told to the owner's apps (0.19.0): board events
//! carry their actor and note, every connection gets a `board_activity` push
//! (the one that asked is told it was its own), a refused move is pushed
//! there and never as a board event, and `bot_state` names the card a turn
//! is about.

use crate::common;

use std::time::Duration;

use bus::contract::board::{self as c, board_request::Request};
use bus::contract::wire::{envelope::Body, Envelope};
use common::board::*;
use common::tasks::project_with_bots;
use common::*;
use prost::Message;
use serde_json::{json, Value};

fn activity(v: &Value) -> bool {
    v["type"] == "board_activity"
}

async fn watch(c: &mut WsClient, project_id: &str) {
    snapshot(
        call(
            c,
            Request::BoardWatch(c::BoardWatch {
                project_id: project_id.to_string(),
            }),
        )
        .await,
    );
}

/// A board with one owner's card on it; the project and the card.
async fn board_with_card(d: &TestDaemon, owner: &mut WsClient) -> (String, String) {
    let project_id = common::peers::project(owner, "Activity").await;
    snapshot(
        call(
            owner,
            Request::BoardGet(c::BoardGet {
                project_id: project_id.clone(),
            }),
        )
        .await,
    );
    let (id, _) = new_item(
        &d.app.db,
        &project_id,
        "Toasts",
        hermesd::kanban::model::Priority::P1,
    );
    (project_id, id)
}

#[tokio::test]
async fn an_owner_move_names_its_actor_and_tells_each_app_whose_it_was() {
    let d = spawn_daemon().await;
    let mut owner = WsClient::connect(&d).await;
    let mut elsewhere = WsClient::connect(&d).await;
    let mut watcher = WsClient::connect(&d).await;
    let (project_id, id) = board_with_card(&d, &mut owner).await;
    watch(&mut watcher, &project_id).await;

    // Sent without waiting for the answer, so the push isn't skipped.
    let version = d.app.db.get_item(&id).unwrap().unwrap().version;
    let request = move_to(&id, "cancelled", version, Some("Folded into H-020"));
    let envelope = Envelope {
        req_id: 77,
        body: Some(Body::BoardRequest(c::BoardRequest {
            request: Some(request),
        })),
    };
    send_raw(&mut owner, envelope.encode_to_vec()).await;

    let event = next_push(&mut watcher).await.expect("a board push");
    assert_eq!(event.kind, c::BoardEventKind::ItemMoved as i32);
    assert_eq!(event.actor.as_deref(), Some("user"));
    assert!(event.note.is_some(), "the move's note: {event:?}");

    let mine = owner.wait_for(activity).await;
    let a = &mine["activity"];
    assert_eq!(a["kind"], "BOARD_ACTIVITY_KIND_MOVED", "{mine}");
    assert_eq!(a["actor"], "user");
    assert_eq!(
        (
            a["item_id"].as_str(),
            a["from_column"].as_str(),
            a["to_column"].as_str()
        ),
        (Some(id.as_str()), Some("todo"), Some("cancelled"))
    );
    assert_eq!(a["item_title"], "Toasts");
    assert_eq!(a["from_this_client"], true);
    assert_eq!(a["title"], format!("You moved {id}"));
    assert!(a["to_name"].as_str().is_some_and(|n| !n.is_empty()));

    let theirs = elsewhere.wait_for(activity).await;
    assert_eq!(theirs["activity"]["item_id"], json!(id));
    assert!(
        theirs["activity"]
            .get("from_this_client")
            .is_none_or(|v| v == false),
        "another app didn't ask for it: {theirs}"
    );
}

#[tokio::test]
async fn a_bots_assignment_move_and_refusal_reach_every_app() {
    let (pair, mut bots) = project_with_bots(&["Team Lead", "Desktop Dev"]).await;
    let [lead, _dev] = &mut bots[..] else {
        unreachable!()
    };
    let d = &pair.d;
    let (lead_id, dev_id) = (pair.ids[0].clone(), pair.ids[1].clone());
    let project = d.app.db.get_bot(&lead_id).unwrap().unwrap().project_id;
    d.app
        .db
        .ensure_board(&project, &d.app.db.daemon_id().unwrap(), Some("H"))
        .unwrap();
    common::roles::by_name(&d.app.db, &project);
    let created = lead
        .call(
            "item_create",
            json!({"type": "feature", "title": "Board toasts", "labels": ["area:daemon"],
                   "size": "M", "acceptance_criteria": ["the owner sees it"]}),
        )
        .await;
    let id = created["item"]["id"].as_str().unwrap().to_string();
    let mut owner = WsClient::connect(d).await;
    let mut watcher = WsClient::connect(d).await;
    watch(&mut watcher, &project).await;

    // Refused: no board event, but every app hears why.
    let raw = lead
        .call_raw(
            "item_move",
            json!({"id": id, "to": "ready", "expected_version": version(&created["item"])}),
        )
        .await;
    assert_eq!(raw["isError"], true, "{raw}");
    let refused = owner.wait_for(activity).await;
    let a = &refused["activity"];
    assert_eq!(a["kind"], "BOARD_ACTIVITY_KIND_MOVE_REFUSED", "{refused}");
    assert_eq!(a["actor"], format!("bot:{lead_id}"));
    assert_eq!(a["actor_name"], "Team Lead");
    assert_eq!(a["to_column"], "ready");
    assert!(
        a["unmet"].as_array().is_some_and(|u| !u.is_empty()),
        "{refused}"
    );
    assert!(
        a["title"]
            .as_str()
            .unwrap()
            .starts_with("Team Lead couldn't move"),
        "{refused}"
    );
    // The refusal itself is an activity, never a move. The board may push
    // the card again, so its "can't move" line shows (card flow, #29).
    while let Some(envelope) = next_envelope(&mut watcher, Duration::from_millis(600)).await {
        if let Some(Body::BoardPush(c::BoardPush {
            push: Some(c::board_push::Push::BoardEvent(event)),
        })) = envelope.body
        {
            assert_ne!(
                event.kind,
                c::BoardEventKind::ItemMoved as i32,
                "a refusal moves nothing: {event:?}"
            );
        }
    }

    // Assigned by the lead.
    let item = lead.call("item_get", json!({ "id": id })).await["item"].clone();
    lead.call(
        "item_assign",
        json!({"id": id, "expected_version": version(&item), "bot": "Desktop Dev"}),
    )
    .await;
    let assigned = owner.wait_for(activity).await;
    let a = &assigned["activity"];
    assert_eq!(a["kind"], "BOARD_ACTIVITY_KIND_ASSIGNED", "{assigned}");
    assert_eq!(a["actor"], format!("bot:{lead_id}"));
    assert_eq!(a["assignee"], json!(dev_id));
    assert_eq!(a["assignee_name"], "Desktop Dev");
    let event = next_push(&mut watcher)
        .await
        .expect("the assignment's push");
    assert_eq!(event.actor, Some(format!("bot:{lead_id}")));

    // Moved by the lead, once the Definition of Ready holds.
    lead.call(
        "item_link",
        json!({"id": id, "kind": "artifact", "ref": "artifacts/spec.md"}),
    )
    .await;
    let item = lead.call("item_get", json!({ "id": id })).await["item"].clone();
    lead.call(
        "item_move",
        json!({"id": id, "to": "ready", "expected_version": version(&item)}),
    )
    .await;
    let moved = owner
        .wait_for(|v| activity(v) && v["activity"]["kind"] == "BOARD_ACTIVITY_KIND_MOVED")
        .await;
    assert_eq!(moved["activity"]["actor"], format!("bot:{lead_id}"));
    assert_eq!(moved["activity"]["to_column"], "ready");
}

#[tokio::test]
async fn a_facts_move_is_the_daemons_with_its_note() {
    let d = spawn_daemon().await;
    let mut owner = WsClient::connect(&d).await;
    let (project_id, id) = board_with_card(&d, &mut owner).await;
    walk(&d.app.db, &id, &["cancelled"], Some("PR merged"));
    {
        let mut feed = d.app.board.writer();
        let moved = [(id.clone(), "inbox".to_string())];
        hermesd::kanban::api::publish_moves(&d.app, &mut feed, &project_id, &moved);
    }
    let pushed = owner.wait_for(activity).await;
    let a = &pushed["activity"];
    assert_eq!(a["actor"], "daemon", "{pushed}");
    assert_eq!(a["note"], "PR merged");
    assert_eq!(a["title"], format!("{id} moved"));
    assert!(
        a["body"].as_str().unwrap().ends_with("PR merged"),
        "{pushed}"
    );
}

#[tokio::test]
async fn bot_state_names_the_card_its_turn_is_about() {
    let d = spawn_daemon().await;
    let mut owner = WsClient::connect(&d).await;
    let (project_id, id) = board_with_card(&d, &mut owner).await;
    let bot = create_bot(&mut owner, &project_id, "Worker").await;
    let bot_id = bot["id"].as_str().unwrap().to_string();
    for _ in 0..400 {
        if d.app.supervisor.state(&bot_id).0 == bus::BotState::Ready {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert_eq!(d.app.supervisor.state(&bot_id).0, bus::BotState::Ready);
    let text = format!("[card {id}] The owner changed {id}");
    let sender = hermesd::messaging::user_sender();
    let dm = hermesd::messaging::Dm::new(&bot_id, &sender, bus::MessageKind::Note, &text);
    let msg = hermesd::messaging::send_dm(&d.app.db, &d.app.events, dm).unwrap();
    owner
        .wait_for_within(Duration::from_secs(10), |v| {
            v["type"] == "delivery_update"
                && v["delivery"]["message_id"] == json!(msg.id)
                && v["delivery"]["state"] == "delivered"
        })
        .await;

    d.app.supervisor.on_hook(&bot_id, "UserPromptSubmit", None);
    let working = owner
        .wait_for(|v| v["type"] == "bot_state" && v["state"] == "working")
        .await;
    assert_eq!(working["item_id"], json!(id), "{working}");
    assert_eq!(working["item_title"], "Toasts");
    d.app.supervisor.on_hook(&bot_id, "Stop", None);
    let done = owner
        .wait_for(|v| v["type"] == "bot_state" && v["state"] == "ready")
        .await;
    assert_eq!(done["item_id"], json!(id), "{done}");

    // The next turn, a chat, is about no card.
    d.app.supervisor.on_hook(&bot_id, "UserPromptSubmit", None);
    let chat = owner
        .wait_for(|v| v["type"] == "bot_state" && v["state"] == "working")
        .await;
    assert!(chat.get("item_id").is_none(), "{chat}");
}
