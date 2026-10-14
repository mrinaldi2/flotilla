//! Attachments and drafts: what is refused, and who sees what (0.19.0 K4b).

use crate::common;

use bus::contract::board::{self as c, board_request::Request, board_response::Response};
use common::board::*;
use common::tasks::{drain_until, error_text, project_with_bots};
use common::{McpClient, TestDaemon, WsClient};
use serde_json::json;

struct Setup {
    project_id: String,
    lead_id: String,
    builder_id: String,
}

fn setup(d: &TestDaemon, ids: &[String]) -> Setup {
    let db = &d.app.db;
    let project_id = db.get_bot(&ids[0]).unwrap().unwrap().project_id;
    db.set_project_lead(&project_id, Some(&ids[0])).unwrap();
    db.ensure_board(&project_id, &db.daemon_id().unwrap(), Some("H"))
        .unwrap();
    Setup {
        project_id,
        lead_id: ids[0].clone(),
        builder_id: ids[1].clone(),
    }
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

fn chunk(item: &str, upload_id: &str, name: &str, data: &[u8], total: u64) -> Request {
    Request::AttachmentUpload(c::AttachmentUpload {
        item_id: item.into(),
        upload_id: upload_id.into(),
        offset: 0,
        data: data.to_vec(),
        total,
        name: name.into(),
    })
}

#[tokio::test]
async fn uploads_over_a_cap_of_a_wrong_type_or_content_are_refused() {
    let (pair, _bots) = project_with_bots(&["Lead", "Builder"]).await;
    let s = setup(&pair.d, &pair.ids);
    let mut owner = WsClient::connect(&pair.d).await;
    let item = create(&mut owner, &s.project_id, "Crash on open", false).await;
    let png = b"\x89PNG\r\n\x1a\n".to_vec();
    let refusals = [
        // Over the 10 MB image cap, refused from the first chunk.
        chunk(&item.id, "big", "huge.png", &png, 11 * 1024 * 1024),
        // A type the allowlist doesn't take.
        chunk(&item.id, "exe", "tool.exe", b"MZ", 2),
        // HEIC, which the app exports as JPEG first.
        chunk(&item.id, "heic", "IMG_1.HEIC", b"....ftypheic", 12),
        // A name that says PNG over content that isn't.
        chunk(&item.id, "fake", "shot.png", b"MZ\x90\0not a png", 14),
        // An upload id that would leave the item's folder.
        chunk(&item.id, "../escape", "shot.png", &png, 8),
    ];
    for request in refusals {
        let code = error_code(call(&mut owner, request).await);
        assert_eq!(code, "invalid_request");
    }
    // A chunk out of order.
    call(&mut owner, chunk(&item.id, "ok", "shot.png", &png, 16)).await;
    let gap = Request::AttachmentUpload(c::AttachmentUpload {
        item_id: item.id.clone(),
        upload_id: "ok".into(),
        offset: 12,
        data: png.clone(),
        total: 16,
        name: "shot.png".into(),
    });
    assert_eq!(error_code(call(&mut owner, gap).await), "invalid_request");
    // A URL must be one.
    let bad_url = Request::ItemAttach(c::ItemAttach {
        id: item.id.clone(),
        url: Some("file:///etc/passwd".into()),
        ..Default::default()
    });
    assert_eq!(
        error_code(call(&mut owner, bad_url).await),
        "invalid_request"
    );
}

#[tokio::test]
async fn drafts_are_hidden_refuse_board_work_and_keep_history_when_discarded() {
    let (pair, mut bots) = project_with_bots(&["Lead", "Builder"]).await;
    let [lead, builder]: &mut [McpClient; 2] = (&mut bots[..]).try_into().expect("two bots");
    let s = setup(&pair.d, &pair.ids);
    let mut owner = WsClient::connect(&pair.d).await;

    // Only the owner starts a draft; every bot lists the draft tools.
    let raw = lead
        .call_raw(
            "item_create",
            json!({"type": "bug", "title": "Mine", "draft": true}),
        )
        .await;
    assert!(error_text(&raw).contains("only the owner"), "{raw}");
    let tools = builder.tools().await.to_string();
    assert!(tools.contains("\"item_attach\"") && tools.contains("\"item_detach\""));

    let draft = create(&mut owner, &s.project_id, "Sort is wrong", true).await;
    let id = draft.id.clone();
    call(
        &mut owner,
        Request::ItemInvite(c::ItemInvite {
            id: id.clone(),
            bot: Some("Lead".into()),
            remove: None,
        }),
    )
    .await;

    // The uninvited bot can't touch it; the invited one can't do board work.
    let raw = builder
        .call_raw(
            "item_update",
            json!({"id": id, "expected_version": 1, "title": "x"}),
        )
        .await;
    assert!(error_text(&raw).contains("no item"), "{raw}");
    for (tool, args) in [
        (
            "item_assign",
            json!({"id": id, "expected_version": 1, "bot": "Builder"}),
        ),
        (
            "item_block",
            json!({"id": id, "expected_version": 1, "reason": "wait"}),
        ),
        ("item_comment", json!({"id": id, "body": "hi"})),
    ] {
        let raw = lead.call_raw(tool, args).await;
        assert!(error_text(&raw).contains("is a draft"), "{tool}: {raw}");
    }
    // Delegating work on it is refused too: it isn't on the board.
    let raw = lead
        .call_raw(
            "send_message",
            json!({"to": "Builder", "kind": "task", "item": id, "body": "do it"}),
        )
        .await;
    assert!(error_text(&raw).contains("draft"), "{raw}");
    // Nor can the owner move it.
    let refused = moved(call(&mut owner, move_to(&id, "ready", draft.version, None)).await);
    assert!(
        matches!(&refused, c::move_result::Outcome::Refused(r)
            if r.unmet.iter().any(|u| u.text.contains("Finalize the draft first"))),
        "{refused:?}"
    );

    // Discarded: out of the tray, its history kept, nothing more to change.
    let discarded = call(
        &mut owner,
        Request::ItemDiscard(c::ItemDiscard {
            id: id.clone(),
            expected_version: draft.version,
            reason: "Not a bug".into(),
        }),
    )
    .await;
    assert!(matches!(
        response(discarded),
        Response::Edited(c::EditResult {
            outcome: Some(c::edit_result::Outcome::Done(ref item)),
            ..
        }) if item.state == "discarded"
    ));
    let board = snapshot(
        call(
            &mut owner,
            Request::BoardGet(c::BoardGet {
                project_id: s.project_id.clone(),
            }),
        )
        .await,
    );
    assert!(board.drafts.is_empty() && !board.cards.iter().any(|card| card.id == id));
    let history = pair.d.app.db.item_events(&id).unwrap();
    assert!(history
        .iter()
        .any(|e| e.field.as_deref() == Some("state") && e.to.as_deref() == Some("discarded")));
    let raw = lead
        .call_raw(
            "item_update",
            json!({"id": id, "expected_version": 2, "title": "x"}),
        )
        .await;
    assert!(error_text(&raw).contains("discarded"), "{raw}");
    let again = Request::ItemFinalize(c::ItemFinalize {
        id: id.clone(),
        expected_version: draft.version + 1,
        column: None,
    });
    assert!(matches!(
        moved(call(&mut owner, again).await),
        c::move_result::Outcome::Refused(_)
    ));
    let _ = (s.lead_id, s.builder_id);
}

#[tokio::test]
async fn the_assignee_hears_of_the_owners_edits_and_a_detached_file_is_a_placeholder() {
    let (pair, mut bots) = project_with_bots(&["Lead", "Builder"]).await;
    let builder = &mut bots[1];
    let s = setup(&pair.d, &pair.ids);
    let mut owner = WsClient::connect(&pair.d).await;
    let item = create(&mut owner, &s.project_id, "Crash on open", false).await;
    let assigned = call(
        &mut owner,
        Request::ItemAssign(c::ItemAssign {
            id: item.id.clone(),
            expected_version: item.version,
            bot: Some("Builder".into()),
        }),
    )
    .await;
    let Response::Edited(c::EditResult {
        outcome: Some(c::edit_result::Outcome::Done(item)),
        ..
    }) = response(assigned)
    else {
        panic!("expected the item assigned");
    };
    assert_eq!(item.assignee.as_deref(), Some(s.builder_id.as_str()));

    // A log, then a reference to it in the description, then detached.
    let log = b"panic at main.rs:1\n";
    call(
        &mut owner,
        chunk(&item.id, "log", "crash.log", log, log.len() as u64),
    )
    .await;
    let attached = call(
        &mut owner,
        Request::ItemAttach(c::ItemAttach {
            id: item.id.clone(),
            upload_id: Some("log".into()),
            name: Some("crash.log".into()),
            ..Default::default()
        }),
    )
    .await;
    let Response::Item(detail) = response(attached) else {
        panic!("expected the item");
    };
    let att = detail.attachments[0].clone();
    let edited = call(
        &mut owner,
        Request::ItemUpdate(c::ItemUpdate {
            id: item.id.clone(),
            expected_version: item.version,
            description: Some(format!("See [the log](attachment:{}).", att.id)),
            ..Default::default()
        }),
    )
    .await;
    assert!(matches!(response(edited), Response::Edited(_)));
    let told = drain_until(builder, "The owner changed").await;
    assert!(
        told.iter()
            .any(|m| m["body"].as_str().is_some_and(|b| b.contains(&item.id))),
        "{told:?}"
    );
    let detached = call(
        &mut owner,
        Request::ItemDetach(c::ItemDetach {
            id: item.id.clone(),
            attachment_id: att.id.clone(),
        }),
    )
    .await;
    let Response::Item(detail) = response(detached) else {
        panic!("expected the item");
    };
    assert!(detail.attachments[0].removed);
    let got = builder.call("item_get", json!({ "id": item.id })).await;
    let shown = got["item"]["description"].as_str().unwrap();
    assert!(shown.contains("[removed attachment: crash.log]"), "{shown}");
    // Its bytes stay readable for the history.
    let read = call(
        &mut owner,
        Request::AttachmentRead(c::AttachmentRead {
            attachment_id: att.id.clone(),
            offset: 0,
            limit: 0,
        }),
    )
    .await;
    let Response::Attachment(chunk) = response(read) else {
        panic!("expected bytes");
    };
    assert_eq!(
        (chunk.data.as_slice(), chunk.total),
        (&log[..], log.len() as u64)
    );
}
