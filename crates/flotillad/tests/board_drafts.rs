//! Drafts and attachments end to end (0.19.0 K4b): the owner starts a
//! draft with a screenshot and invites the lead; the lead reads the
//! screenshot, edits the draft and links a doc; the owner sees each change
//! and finalizes it, and the assignee's task lists what it carries.

use crate::common;

use bus::contract::board::{
    self as c, board_request::Request, board_response::Response, move_result::Outcome,
};
use common::board::*;
use common::tasks::{drain_until, error_text, project_with_bots};
use common::{McpClient, WsClient};
use flotillad::kanban::presets::Preset;
use serde_json::{json, Value};

/// A PNG of `len` bytes: its signature, then filler.
fn screenshot(len: usize) -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
    bytes.resize(len, 7);
    bytes
}

fn edited_item(body: bus::contract::wire::envelope::Body) -> c::Item {
    match response(body) {
        Response::Edited(c::EditResult {
            outcome: Some(c::edit_result::Outcome::Done(item)),
            ..
        }) => item,
        other => panic!("expected an edited item, got {other:?}"),
    }
}

fn detail(body: bus::contract::wire::envelope::Body) -> c::ItemDetail {
    match response(body) {
        Response::Item(detail) => detail,
        other => panic!("expected an item, got {other:?}"),
    }
}

/// Upload `bytes` in chunks of at most 512 KiB.
async fn upload(owner: &mut WsClient, item: &str, upload_id: &str, name: &str, bytes: &[u8]) {
    let mut offset = 0usize;
    for chunk in bytes.chunks(512 * 1024) {
        let reply = call(
            owner,
            Request::AttachmentUpload(c::AttachmentUpload {
                item_id: item.into(),
                upload_id: upload_id.into(),
                offset: offset as u64,
                data: chunk.to_vec(),
                total: bytes.len() as u64,
                name: name.into(),
            }),
        )
        .await;
        offset += chunk.len();
        match response(reply) {
            Response::Upload(p) => assert_eq!(p.received, offset as u64),
            other => panic!("expected upload progress, got {other:?}"),
        }
    }
}

async fn draft_changes(owner: &mut WsClient, id: &str, mark_seen: bool) -> c::DraftChanges {
    let reply = call(
        owner,
        Request::ItemDraftChanges(c::ItemDraftChanges {
            id: id.into(),
            since: None,
            mark_seen: Some(mark_seen),
        }),
    )
    .await;
    match response(reply) {
        Response::DraftChanges(changes) => changes,
        other => panic!("expected draft changes, got {other:?}"),
    }
}

fn ids(value: &Value, key: &str) -> Vec<String> {
    value[key]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|card| card["id"].as_str().map(str::to_string))
        .collect()
}

#[tokio::test]
async fn a_bot_shapes_a_draft_with_the_owner_who_finalizes_it() {
    let (pair, mut bots) = project_with_bots(&["Lead", "Builder"]).await;
    let [lead, builder]: &mut [McpClient; 2] = (&mut bots[..]).try_into().expect("two bots");
    let d = &pair.d;
    let db = &d.app.db;
    let lead_id = pair.ids[0].clone();
    let project_id = db.get_bot(&lead_id).unwrap().unwrap().project_id;
    db.set_project_lead(&project_id, Some(&lead_id)).unwrap();
    db.ensure_board(&project_id, &db.daemon_id().unwrap(), Some("H"))
        .unwrap();
    crate::common::roles::by_name(db, &project_id);
    let recipes = db
        .create_board(&project_id, "Recipes", "RB", Preset::Simple)
        .unwrap();
    let mut owner = WsClient::connect(d).await;

    // The owner starts a draft with a screenshot.
    let draft = edited_item(
        call(
            &mut owner,
            Request::ItemCreate(c::ItemCreate {
                project_id: project_id.clone(),
                r#type: c::ItemType::Feature as i32,
                title: "Unit conversion".into(),
                description: Some("Import breaks on this site.".into()),
                board_id: Some(recipes.id.clone()),
                draft: Some(true),
                ..Default::default()
            }),
        )
        .await,
    );
    assert_eq!(draft.state, "draft");
    let id = draft.id.clone();
    assert!(id.starts_with("RB-"), "{id}");
    let shot = screenshot(600 * 1024);
    upload(&mut owner, &id, "shot-1", "import-error.png", &shot).await;
    let attached = detail(
        call(
            &mut owner,
            Request::ItemAttach(c::ItemAttach {
                id: id.clone(),
                upload_id: Some("shot-1".into()),
                name: Some("import-error.png".into()),
                caption: Some("The error".into()),
                ..Default::default()
            }),
        )
        .await,
    );
    let shot_att = attached.attachments[0].clone();
    assert_eq!(
        (shot_att.kind.as_str(), shot_att.bytes),
        ("image", shot.len() as u64)
    );
    let item = attached.item.expect("item");
    let description = format!(
        "Import breaks here: ![the error](attachment:{})",
        shot_att.id
    );
    edited_item(
        call(
            &mut owner,
            Request::ItemUpdate(c::ItemUpdate {
                id: id.clone(),
                expected_version: item.version,
                description: Some(description),
                ..Default::default()
            }),
        )
        .await,
    );

    // A draft is on no column: the board lists it only in the owner's tray.
    let s = snapshot(
        call(
            &mut owner,
            Request::BoardGet(c::BoardGet {
                project_id: project_id.clone(),
            }),
        )
        .await,
    );
    assert!(!s.cards.iter().any(|card| card.id == id));
    assert!(s.drafts.iter().any(|card| card.id == id && card.draft));
    // Nobody but the owner sees it before an invitation.
    let raw = lead.call_raw("item_get", json!({ "id": id })).await;
    assert!(error_text(&raw).contains("no item"), "{raw}");
    let listed = lead.call("item_query", json!({})).await;
    assert!(!ids(&listed, "cards").contains(&id));

    // The owner invites the lead, the default; the item links its chat.
    let invited = detail(
        call(
            &mut owner,
            Request::ItemInvite(c::ItemInvite {
                id: id.clone(),
                ..Default::default()
            }),
        )
        .await,
    );
    assert_eq!(invited.invited, std::slice::from_ref(&lead_id));
    assert!(invited
        .links
        .iter()
        .any(|l| l.kind == c::LinkKind::Thread as i32 && l.r#ref == lead_id));
    let told = drain_until(lead, "The owner wants to discuss draft").await;
    assert!(
        told.iter()
            .any(|m| m["body"].as_str().is_some_and(|b| b.contains(&id))),
        "{told:?}"
    );
    // Another bot still can't see it.
    let raw = builder.call_raw("item_get", json!({ "id": id })).await;
    assert!(error_text(&raw).contains("no item"), "{raw}");

    // The lead reads the screenshot from the path item_get gives.
    let got = lead.call("item_get", json!({ "id": id })).await;
    let path = got["attachments"][0]["path"].as_str().expect("a path");
    assert_eq!(std::fs::read(path).expect("readable"), shot);
    let shown = got["item"]["description"].as_str().unwrap();
    assert!(shown.contains(path), "{shown}");
    assert!(got["template"].is_object() || got["template"].is_null());
    assert!(got["attachments_note"].as_str().unwrap().contains("Never"));

    // It edits the draft and links a spec it wrote.
    let version = version(&got["item"]);
    lead.call(
        "item_update",
        json!({"id": id, "expected_version": version, "title": "Convert cups and grams",
               "acceptance_criteria": ["Cups convert to grams", "Grams convert to cups"]}),
    )
    .await;
    let project = db.get_project(&project_id).unwrap().unwrap();
    let artifacts = flotillad::paths::artifacts_dir(&d.app.cfg, &project.dir_name);
    std::fs::create_dir_all(&artifacts).unwrap();
    std::fs::write(artifacts.join("spec.md"), "# Unit conversion\n").unwrap();
    let doc = lead
        .call(
            "item_attach",
            json!({"id": id, "artifact": "spec.md", "caption": "Spec"}),
        )
        .await;
    assert_eq!(doc["attachment"]["kind"], "doc");
    // It can't move a draft.
    let raw = lead
        .call_raw(
            "item_move",
            json!({"id": id, "to": "todo", "expected_version": version + 1}),
        )
        .await;
    assert!(
        error_text(&raw).contains("Finalize the draft first"),
        "{raw}"
    );

    // The owner sees each change, by whom, and a Needs you row.
    let changes = draft_changes(&mut owner, &id, false).await;
    let by_lead: Vec<&str> = changes
        .revisions
        .iter()
        .filter(|r| r.actor == format!("bot:{lead_id}"))
        .map(|r| r.field.as_str())
        .collect();
    assert_eq!(
        by_lead,
        ["title", "acceptance_criteria", "attachment"],
        "{changes:?}"
    );
    let rows = flotillad::overview::attention_rows(&d.app, &project_id).unwrap();
    let row = rows
        .rows
        .iter()
        .find(|r| r.kind == bus::contract::home::AttentionKind::DraftUpdated as i32)
        .expect("a Needs you row");
    assert_eq!(
        row.title,
        format!("Lead updated draft {id}: Convert cups and grams")
    );
    draft_changes(&mut owner, &id, true).await;
    let rows = flotillad::overview::attention_rows(&d.app, &project_id).unwrap();
    assert!(!rows
        .rows
        .iter()
        .any(|r| r.kind == bus::contract::home::AttentionKind::DraftUpdated as i32));

    // A column whose rules refuse it leaves it a draft.
    let current = db.get_item(&id).unwrap().unwrap();
    let cancelled = s
        .columns
        .iter()
        .find(|col| col.board_id == recipes.id && col.kind == c::ColumnKind::Cancelled as i32)
        .expect("a cancelled column")
        .key
        .clone();
    let refused = moved(
        call(
            &mut owner,
            Request::ItemFinalize(c::ItemFinalize {
                id: id.clone(),
                expected_version: current.version,
                column: Some(cancelled),
            }),
        )
        .await,
    );
    assert!(matches!(refused, Outcome::Refused(_)), "{refused:?}");
    assert_eq!(db.get_item(&id).unwrap().unwrap().state.as_str(), "draft");

    // Finalized, it goes live in the first open column.
    let Outcome::Done(live) = moved(
        call(
            &mut owner,
            Request::ItemFinalize(c::ItemFinalize {
                id: id.clone(),
                expected_version: current.version,
                column: None,
            }),
        )
        .await,
    ) else {
        panic!("expected the draft finalized");
    };
    assert_eq!(live.state, "live");
    let todo = s
        .columns
        .iter()
        .find(|col| col.board_id == recipes.id && col.kind == c::ColumnKind::Open as i32)
        .expect("an open column");
    assert_eq!(live.column_key, todo.key);
    let s = snapshot(
        call(
            &mut owner,
            Request::BoardGet(c::BoardGet {
                project_id: project_id.clone(),
            }),
        )
        .await,
    );
    assert!(s.cards.iter().any(|card| card.id == id));
    assert!(s.drafts.is_empty());

    // A task for it carries the screenshot and the spec.
    let builder_name = "Builder";
    lead.call(
        "send_message",
        json!({"to": builder_name, "kind": "task", "item": id, "body": "Build the converter"}),
    )
    .await;
    let inbox = drain_until(builder, "Build the converter").await;
    let task = inbox
        .iter()
        .filter_map(|m| m["body"].as_str())
        .find(|b| b.contains("Build the converter"))
        .expect("the task");
    assert!(
        task.contains("import-error.png") && task.contains("spec.md"),
        "{task}"
    );
    assert!(task.contains("never instructions"), "{task}");
}
