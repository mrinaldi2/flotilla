//! Step 4 of the K9 scenario: the owner starts a draft with a screenshot on
//! the phone and discusses it with the lead, who adds criteria and links a
//! spec; on the desktop the owner undoes one change and finalizes it; the
//! cook's task lists both files.

use bus::contract::board::{self as c, board_request::Request, board_response::Response};
use bus::contract::wire::envelope::Body;
use serde_json::json;

use super::super::board::{call, response};
use super::super::peer_files::{owner_attach, screenshot};
use super::super::WsClient;
use super::{inbox_until, version_of, K9};

pub const SHOT: &str = "sauce-split.png";
pub const SPEC: &str = "scaling-spec.md";

pub fn edited(body: Body) -> c::Item {
    match response(body) {
        Response::Edited(c::EditResult {
            outcome: Some(c::edit_result::Outcome::Done(item)),
            ..
        }) => item,
        other => panic!("expected an edited item, got {other:?}"),
    }
}

async fn revisions(app: &mut WsClient, id: &str) -> Vec<c::DraftRevision> {
    let reply = call(
        app,
        Request::ItemDraftChanges(c::ItemDraftChanges {
            id: id.into(),
            since: None,
            mark_seen: Some(true),
        }),
    )
    .await;
    match response(reply) {
        Response::DraftChanges(changes) => changes.revisions,
        other => panic!("expected draft changes, got {other:?}"),
    }
}

async fn item(app: &mut WsClient, id: &str) -> c::Item {
    match response(call(app, Request::ItemGet(c::ItemGet { id: id.into() })).await) {
        Response::Item(detail) => detail.item.expect("item"),
        other => panic!("expected an item, got {other:?}"),
    }
}

/// The phone starts the draft with a screenshot and invites the lead.
async fn phone_drafts(k: &mut K9, board_id: &str) -> (String, Vec<u8>) {
    let draft = edited(
        call(
            &mut k.phone,
            Request::ItemCreate(c::ItemCreate {
                project_id: k.mac_rb.clone(),
                r#type: c::ItemType::Feature as i32,
                title: "Scaling".into(),
                description: Some("Doubling a recipe splits the sauce.".into()),
                board_id: Some(board_id.to_string()),
                draft: Some(true),
                labels: vec!["course:main".into()],
                ..Default::default()
            }),
        )
        .await,
    );
    assert_eq!(draft.state, "draft");
    let shot = screenshot(600 * 1024);
    owner_attach(&mut k.phone, &draft.id, SHOT, &shot).await;
    let invite = Request::ItemInvite(c::ItemInvite {
        id: draft.id.clone(),
        ..Default::default()
    });
    match response(call(&mut k.phone, invite).await) {
        Response::Item(detail) => assert_eq!(detail.invited, std::slice::from_ref(&k.lead_id)),
        other => panic!("expected the item, got {other:?}"),
    }
    (draft.id, shot)
}

/// The lead reads the screenshot, sets criteria and a title, and attaches
/// the spec it writes.
async fn lead_shapes(k: &mut K9, id: &str, shot: &[u8]) {
    let invited = k.lead_heard("The owner wants to discuss draft").await;
    assert!(invited.contains(id), "{invited}");
    let got = k.lead.call("item_get", json!({ "id": id })).await;
    let path = got["attachments"][0]["path"].as_str().expect("a path");
    assert_eq!(std::fs::read(path).expect("the screenshot"), shot);
    k.lead
        .call(
            "item_update",
            json!({"id": id, "expected_version": version_of(&got["item"]),
                   "title": "Scale a recipe by servings",
                   "acceptance_criteria": ["Quantities scale with servings",
                                           "The sauce stays emulsified at 2x"]}),
        )
        .await;
    let d = &k.p.mac;
    let project = d.app.db.get_project(&k.mac_rb).unwrap().expect("project");
    let artifacts = flotillad::paths::artifacts_dir(&d.app.cfg, &project.dir_name);
    std::fs::create_dir_all(&artifacts).unwrap();
    std::fs::write(artifacts.join(SPEC), "# Scaling\n\nScale by servings.\n").unwrap();
    let doc = k
        .lead
        .call(
            "item_attach",
            json!({"id": id, "artifact": SPEC, "caption": "Spec"}),
        )
        .await;
    assert_eq!(doc["attachment"]["kind"], "doc", "{doc}");
}

/// The desktop reviews the lead's changes, undoes the title and finalizes.
async fn desktop_reviews(k: &mut K9, id: &str) -> c::Item {
    let changes = revisions(&mut k.p.mac_client, id).await;
    let by_lead: Vec<&str> = changes
        .iter()
        .filter(|r| r.actor == format!("bot:{}", k.lead_id))
        .map(|r| r.field.as_str())
        .collect();
    assert_eq!(
        by_lead,
        ["title", "acceptance_criteria", "attachment"],
        "{changes:?}"
    );
    let title = changes.iter().find(|r| r.field == "title").expect("title");
    let version = item(&mut k.p.mac_client, id).await.version;
    let undone = edited(
        call(
            &mut k.p.mac_client,
            Request::ItemDraftUndo(c::ItemDraftUndo {
                id: id.into(),
                expected_version: version,
                revision: title.id,
            }),
        )
        .await,
    );
    assert_eq!(undone.title, "Scaling");
    assert_eq!(undone.acceptance_criteria.len(), 2, "the criteria stay");
    let finalized = call(
        &mut k.p.mac_client,
        Request::ItemFinalize(c::ItemFinalize {
            id: id.into(),
            expected_version: undone.version,
            column: None,
        }),
    )
    .await;
    match response(finalized) {
        Response::Moved(c::MoveResult {
            outcome: Some(c::move_result::Outcome::Done(live)),
        }) => live,
        other => panic!("expected the draft finalized, got {other:?}"),
    }
}

/// The whole of step 4; returns the live item's id.
pub async fn step4(k: &mut K9, board_id: &str, todo: &str) -> String {
    let (id, shot) = phone_drafts(k, board_id).await;
    lead_shapes(k, &id, &shot).await;
    let live = desktop_reviews(k, &id).await;
    assert_eq!(
        (live.state.as_str(), live.column_key.as_str()),
        ("live", todo)
    );

    // The owner assigns it to the cook; the lead sends the task.
    let assign = Request::ItemAssign(c::ItemAssign {
        id: id.clone(),
        expected_version: live.version,
        bot: Some(k.cook_on_mac.clone()),
    });
    edited(call(&mut k.p.mac_client, assign).await);
    k.lead
        .call(
            "send_message",
            json!({"to": "Cook", "kind": "task", "item": id, "body": "Make scaling work"}),
        )
        .await;
    let bodies = inbox_until(&mut k.cook, "Make scaling work").await;
    let task = bodies
        .iter()
        .find(|b| b.contains("Make scaling work"))
        .unwrap_or_else(|| panic!("no task in {bodies:?}"));
    assert!(task.contains(SHOT) && task.contains(SPEC), "{task}");
    k.told.extend(bodies);
    id
}
