//! Where attachment bytes may come from, and how many an item may keep
//! (review of K4b): detached files still count, since their bytes stay;
//! nothing reached through a link outside the project is copied or linked.

use crate::common;

use std::path::PathBuf;

use bus::contract::board::{self as c, board_request::Request, board_response::Response};
use common::board::*;
use common::tasks::{error_text, project_with_bots_on};
use common::{spawn_daemon_with, McpClient, TestDaemon, WsClient};
use serde_json::{json, Value};

const KB: usize = 1024;

struct Draft {
    project_id: String,
    id: String,
    workspace: PathBuf,
    // Read only by the symlink tests, which are Unix-only.
    #[cfg_attr(not(unix), allow(dead_code))]
    artifacts: PathBuf,
}

/// A draft the lead is invited to, on a daemon whose items hold 1 MB.
async fn draft() -> (TestDaemon, McpClient, WsClient, Draft) {
    let d = spawn_daemon_with(|cfg| cfg.attachments.item_mb = 1).await;
    let (pair, mut bots) = project_with_bots_on(d, &["Lead"]).await;
    let d = pair.d;
    let db = &d.app.db;
    let lead = db.get_bot(&pair.ids[0]).unwrap().unwrap();
    let project_id = lead.project_id.clone();
    db.ensure_board(&project_id, &db.daemon_id().unwrap(), Some("H"))
        .unwrap();
    crate::common::roles::by_name(db, &project_id);
    let mut owner = WsClient::connect(&d).await;
    let reply = call(
        &mut owner,
        Request::ItemCreate(c::ItemCreate {
            project_id: project_id.clone(),
            r#type: c::ItemType::Bug as i32,
            title: "Import breaks".into(),
            draft: Some(true),
            ..Default::default()
        }),
    )
    .await;
    let Response::Edited(c::EditResult {
        outcome: Some(c::edit_result::Outcome::Done(item)),
        ..
    }) = response(reply)
    else {
        panic!("expected the draft");
    };
    let invite = Request::ItemInvite(c::ItemInvite {
        id: item.id.clone(),
        bot: Some("Lead".into()),
        remove: None,
    });
    call(&mut owner, invite).await;
    let artifacts = hermesd::kanban::attach::artifacts_of(&d.app, &project_id).unwrap();
    std::fs::create_dir_all(&artifacts).unwrap();
    let draft = Draft {
        project_id,
        id: item.id,
        workspace: PathBuf::from(&lead.workspace_path),
        artifacts,
    };
    (d, bots.remove(0), owner, draft)
}

fn text(fill: u8, len: usize) -> Vec<u8> {
    vec![fill; len]
}

async fn attach_file(lead: &mut McpClient, id: &str, file: &str) -> Value {
    lead.call_raw("item_attach", json!({"id": id, "file": file}))
        .await
}

async fn attached(lead: &mut McpClient, id: &str, file: &str) -> String {
    let reply = lead
        .call("item_attach", json!({"id": id, "file": file}))
        .await;
    reply["attachment"]["id"]
        .as_str()
        .expect("an id")
        .to_string()
}

#[tokio::test]
async fn detached_files_count_for_the_item_since_their_bytes_stay() {
    let (_d, mut lead, _owner, draft) = draft().await;
    std::fs::write(draft.workspace.join("a.txt"), text(b'a', 600 * KB)).unwrap();
    std::fs::write(draft.workspace.join("b.txt"), text(b'b', 600 * KB)).unwrap();
    let first = attached(&mut lead, &draft.id, "a.txt").await;
    lead.call(
        "item_detach",
        json!({"id": draft.id, "attachment_id": first}),
    )
    .await;
    // Other bytes would hold 1.2 MB on disk for a 1 MB item.
    let refused = attach_file(&mut lead, &draft.id, "b.txt").await;
    assert!(error_text(&refused).contains("at most 1 MB"), "{refused}");
    // The same bytes again take no more room.
    attached(&mut lead, &draft.id, "a.txt").await;
    let _ = draft.project_id;
}

#[cfg(unix)]
#[tokio::test]
async fn a_link_out_of_the_project_is_neither_linked_nor_copied() {
    let (_d, mut lead, mut owner, draft) = draft().await;
    let outside = tempfile::tempdir().unwrap();
    let secret = outside.path().join("secret.md");
    std::fs::write(&secret, b"the daemon's secret\n").unwrap();

    // An artifact that is a link out of the artifacts folder.
    std::os::unix::fs::symlink(&secret, draft.artifacts.join("notes.md")).unwrap();
    let raw = lead
        .call_raw(
            "item_attach",
            json!({"id": draft.id, "artifact": "notes.md"}),
        )
        .await;
    assert!(error_text(&raw).contains("outside"), "{raw}");

    // An upload of the owner's swapped for a link before it is attached.
    let first = Request::AttachmentUpload(c::AttachmentUpload {
        item_id: draft.id.clone(),
        upload_id: "u1".into(),
        offset: 0,
        data: b"my notes\n".to_vec(),
        total: 9,
        name: "notes.md".into(),
    });
    assert!(matches!(
        response(call(&mut owner, first).await),
        Response::Upload(_)
    ));
    let staged = staged_upload(&draft.artifacts, &draft.id);
    std::fs::remove_file(&staged).unwrap();
    std::os::unix::fs::symlink(&secret, &staged).unwrap();
    let attach = Request::ItemAttach(c::ItemAttach {
        id: draft.id.clone(),
        upload_id: Some("u1".into()),
        name: Some("notes.md".into()),
        ..Default::default()
    });
    assert_eq!(
        error_code(call(&mut owner, attach).await),
        "invalid_request"
    );
    let stored = std::fs::read_dir(staged.parent().unwrap().parent().unwrap())
        .unwrap()
        .filter_map(Result::ok)
        .filter(|e| e.file_name() != ".uploads")
        .count();
    assert_eq!(stored, 0, "nothing was copied in");
}

/// The one staged upload of an item.
#[cfg(unix)]
fn staged_upload(artifacts: &std::path::Path, item: &str) -> PathBuf {
    let dir = artifacts.join("items").join(item).join(".uploads");
    let mut entries: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .collect();
    assert_eq!(entries.len(), 1, "{entries:?}");
    entries.remove(0)
}
