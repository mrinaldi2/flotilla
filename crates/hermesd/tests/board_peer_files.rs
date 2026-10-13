//! An item's files across linked computers (0.19.0 K6): a bot on the PC
//! attaches a file of its own by sending its bytes, under the PC's caps, and
//! the Mac, which holds the board, never opens a path a peer names nor
//! serves a draft's files or another project's.

use crate::common;

use bus::contract::board::{self as c, board_request::Request, board_response::Response};
use common::board::{call, response};
use common::peer_board::{board_on, Board};
use common::peer_files::{owner_attach, owner_link_doc, screenshot, stored};
use common::peers::{computer, paired_from, project};
use common::tasks::error_text;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

/// The board fixture, with the PC taking images of at most 1 MB.
async fn small_pc() -> Board {
    let mac = computer(|_| {}).await;
    let win = computer(|cfg| cfg.attachments.image_mb = 1).await;
    board_on(paired_from(mac, win).await).await
}

async fn ask(b: &Board, frame: Value) -> Result<Value, String> {
    b.p.win
        .app
        .peers
        .request(&b.p.win_peer_id, frame)
        .await
        .map_err(|e| format!("{:?}", e))
}

#[tokio::test]
async fn a_linked_bot_attaches_its_own_file_by_sending_it_under_the_caps() {
    let mut b = small_pc().await;
    let tester = b.p.win.app.db.get_bot(&b.tester_id).unwrap().unwrap();
    let workspace = std::path::Path::new(&tester.workspace_path);
    let shot = screenshot(300 * 1024);
    std::fs::write(workspace.join("shot.png"), &shot).unwrap();
    let attached = b
        .tester
        .call(
            "item_attach",
            json!({"id": b.item, "file": "shot.png", "caption": "on win"}),
        )
        .await;
    let sha = hex::encode(Sha256::digest(&shot));
    let att = &attached["attachment"];
    assert_eq!(att["sha256"], sha, "{attached}");
    let path = att["path"].as_str().expect("path");
    assert!(path.contains(&sha), "a copy on the PC: {attached}");
    assert_eq!(std::fs::read(path).unwrap(), shot);
    let id = att["id"].as_str().unwrap();
    let on_mac = stored(&b.p.mac, &b.mac_app, id);
    assert_eq!(
        std::fs::read(on_mac).unwrap(),
        shot,
        "the bytes are on the Mac"
    );
    let record = b.p.mac.app.db.board_read(|t| t.attachment(id)).unwrap();
    let record = record.expect("recorded");
    assert_eq!(record.added_by, format!("bot:{}", b.stand_in));
    assert_eq!(record.caption, "on win");

    // Over the PC's cap, or outside the bot's folders: refused before
    // anything is sent.
    std::fs::write(workspace.join("big.png"), screenshot(2 * 1024 * 1024)).unwrap();
    let raw = b
        .tester
        .call_raw("item_attach", json!({"id": b.item, "file": "big.png"}))
        .await;
    assert!(error_text(&raw).contains("at most 1 MB"), "{raw}");
    let outside = tempfile::tempdir().unwrap();
    let secret = outside.path().join("secret.txt");
    std::fs::write(&secret, "not yours").unwrap();
    let raw = b
        .tester
        .call_raw(
            "item_attach",
            json!({"id": b.item, "file": secret.display().to_string()}),
        )
        .await;
    assert!(error_text(&raw).contains("outside"), "{raw}");
    assert_eq!(
        b.p.mac
            .app
            .db
            .board_read(|t| t.attachments(&b.item))
            .unwrap()
            .len(),
        1
    );

    // An image over the PC's cap stays on the Mac; the one under it is read.
    let big = owner_attach(
        &mut b.p.mac_client,
        &b.item,
        "big.png",
        &screenshot(2 << 20),
    )
    .await;
    let got = b.tester.call("item_get", json!({"id": b.item})).await;
    let listed = got["attachments"].as_array().expect("attachments");
    let over = listed.iter().find(|a| a["id"] == big.id).expect("listed");
    assert_eq!(over["path"], "");
    assert!(
        over["note"].as_str().unwrap().contains("at most 1 MB"),
        "{got}"
    );
    let under = listed.iter().find(|a| a["id"] == id).expect("listed");
    assert_eq!(under["path"], path, "a cache hit: {got}");
}

#[tokio::test]
async fn the_home_opens_no_peer_path_and_serves_no_draft_or_other_project() {
    let mut b = small_pc().await;
    let call_frame = |args: Value| {
        json!({"type": "board_call", "project_id": b.win_app, "bot_id": b.tester_id,
               "tool": "item_attach", "args": args})
    };
    let refused = ask(&b, call_frame(json!({"id": b.item, "file": "/etc/hosts"}))).await;
    assert!(
        refused
            .as_ref()
            .unwrap_err()
            .contains("can't read a file on your computer"),
        "{refused:?}"
    );
    let refused = ask(&b, call_frame(json!({"id": b.item, "upload_id": "up-1"}))).await;
    assert!(refused.unwrap_err().contains("forbidden"));
    let push = json!({
        "type": "attachment_push", "project_id": b.win_app, "bot_id": b.tester_id,
        "item_id": b.item, "upload_id": "peer-0000000000000000-x", "name": "a.png",
        "offset": 0, "total": 8, "data": "iVBORw0KGgo=",
    });
    assert!(ask(&b, push).await.unwrap_err().contains("forbidden"));

    // A live item's file is served, with the sha256 it must hash to.
    let shot = screenshot(1000);
    let live = owner_attach(&mut b.p.mac_client, &b.item, "a.png", &shot).await;
    let fetch = |id: &str, project: &str| {
        json!({"type": "attachment_fetch", "project_id": project, "attachment_id": id,
               "offset": 0})
    };
    let served = ask(&b, fetch(&live.id, &b.win_app)).await.expect("served");
    assert_eq!(served["sha256"], live.sha256);
    assert_eq!(served["total"], 1000);

    // A draft's isn't.
    let created = call(
        &mut b.p.mac_client,
        Request::ItemCreate(c::ItemCreate {
            project_id: b.mac_app.clone(),
            r#type: c::ItemType::Bug as i32,
            title: "Not on the board yet".into(),
            draft: Some(true),
            ..Default::default()
        }),
    )
    .await;
    let draft = match response(created) {
        Response::Edited(c::EditResult {
            outcome: Some(c::edit_result::Outcome::Done(item)),
            ..
        }) => item.id,
        other => panic!("expected the draft, got {other:?}"),
    };
    let in_draft = owner_attach(&mut b.p.mac_client, &draft, "d.png", &shot).await;
    let refused = ask(&b, fetch(&in_draft.id, &b.win_app)).await.unwrap_err();
    assert!(refused.contains("not_found"), "{refused}");

    // Nor a doc, which has no bytes of its own.
    let doc = owner_link_doc(&mut b.p.mac_client, &b.item, "https://example.com/spec").await;
    let refused = ask(&b, fetch(&doc, &b.win_app)).await.unwrap_err();
    assert!(refused.contains("no bytes"), "{refused}");

    // Nor another project's, asked through a linked one or an unlinked one.
    let other = project(&mut b.p.mac_client, "other").await;
    let db = &b.p.mac.app.db;
    db.ensure_board(&other, &db.daemon_id().unwrap(), Some("X"))
        .unwrap();
    let (item, _) = common::board::new_item(
        db,
        &other,
        "Elsewhere",
        hermesd::kanban::model::Priority::P2,
    );
    let theirs = owner_attach(&mut b.p.mac_client, &item, "x.png", &shot).await;
    let refused = ask(&b, fetch(&theirs.id, &b.win_app)).await.unwrap_err();
    assert!(refused.contains("not_found"), "{refused}");
    let unlinked = project(&mut b.p.win_client, "unlinked").await;
    let refused = ask(&b, fetch(&theirs.id, &unlinked)).await.unwrap_err();
    assert!(refused.contains("not_linked"), "{refused}");
}
