//! Boards and their files across linked computers (0.19.0 K6, acceptance):
//! a lead on the PC changes the boards the Mac holds, under the Mac's
//! structure policy, edits an item there and reads its screenshot from a
//! copy on the PC, fetched once and kept by its sha256.

use crate::common;

use bus::contract::board as c;
use common::peer_board::board;
use common::peer_files::{owner_attach, screenshot, stored};
use common::peers::wait_until;
use common::tasks::error_text;
use flotillad::actor::Actor;
use flotillad::kanban::admin::{self, By, Structure};
use flotillad::kanban::model::{ProjectRole, Role};
use flotillad::kanban::presets::Preset;
use serde_json::{json, Value};

fn research(boards: &Value) -> Value {
    boards["boards"]
        .as_array()
        .expect("boards")
        .iter()
        .find(|b| b["board"]["name"] == "Research")
        .cloned()
        .expect("the Research board")
}

#[tokio::test]
async fn a_linked_bot_edits_a_board_homed_elsewhere_and_reads_its_screenshot() {
    let mut b = board().await;
    let (mac, win) = (&b.p.mac.app, &b.p.win.app);
    mac.db
        .set_project_role(&ProjectRole {
            project_id: b.mac_app.clone(),
            role: Role::Lead,
            bot_id: b.stand_in.clone(),
            machine: Some("win".into()),
        })
        .unwrap();

    // The owner adds a board on the Mac; the PC mirrors that board.
    admin::structure(
        mac,
        By::Owner(Actor::User),
        Structure::Create(c::BoardCreate {
            project_id: b.mac_app.clone(),
            name: "Research".into(),
            key: Some("RS".into()),
            preset: Preset::Simple.name().into(),
            ..Default::default()
        }),
    )
    .expect("created");
    let win_app = b.win_app.clone();
    wait_until("the PC mirrors the new board", || {
        win.board_mirror
            .get(&win_app)
            .is_some_and(|m| m.snapshot.boards.iter().any(|b| b.name == "Research"))
    })
    .await;

    // The linked lead proposes a column; the Mac's owner_confirms decides.
    let rs = research(&b.tester.call("board_list", json!({})).await);
    let rs_id = rs["board"]["id"].as_str().expect("id").to_string();
    let proposed = b
        .tester
        .call(
            "column_add",
            json!({"board_id": rs_id, "version": rs["board"]["version"], "name": "QA",
                   "kind": "active"}),
        )
        .await;
    assert_eq!(proposed["applied"], false, "{proposed}");
    let proposal = proposed["proposal_id"].as_str().expect("proposal");
    assert!(mac.db.board_proposal(proposal).unwrap().is_some());
    admin::rule(mac, &Actor::User, proposal, true, None).expect("approved");
    wait_until("the PC mirrors the QA column", || {
        win.board_mirror.get(&win_app).is_some_and(|m| {
            m.snapshot
                .columns
                .iter()
                .any(|col| col.name == "QA" && col.board_id == rs_id)
        })
    })
    .await;

    // It files an item on that board, which lands on the Mac.
    let created = b
        .tester
        .call(
            "item_create",
            json!({"board_id": rs_id, "type": "task", "title": "Read the logs"}),
        )
        .await;
    let rs_item = created["item"]["id"].as_str().expect("id").to_string();
    assert!(rs_item.starts_with("RS-"), "{created}");
    wait_until("the PC mirrors the new item", || {
        win.board_mirror.get(&win_app).is_some_and(|m| {
            m.snapshot
                .cards
                .iter()
                .any(|card| card.id == rs_item && card.board_id == rs_id)
        })
    })
    .await;

    // The owner attaches a screenshot on the Mac; the lead points the
    // description at it.
    let shot = screenshot(700 * 1024);
    let att = owner_attach(&mut b.p.mac_client, &b.item, "error.png", &shot).await;
    let item = b.tester.call("item_get", json!({"id": b.item})).await;
    let edited = b
        .tester
        .call(
            "item_update",
            json!({"id": b.item, "expected_version": item["item"]["version"],
                   "description": format!("See ![the error](attachment:{})", att.id)}),
        )
        .await;
    assert!(edited["item"].is_object(), "{edited}");

    // It reads the screenshot from a copy on the PC.
    let got = b.tester.call("item_get", json!({"id": b.item})).await;
    let listed = &got["attachments"][0];
    let path = listed["path"].as_str().expect("path");
    assert!(path.contains(&att.sha256), "{got}");
    let win_project = win.db.get_project(&b.win_app).unwrap().unwrap();
    let artifacts = flotillad::paths::artifacts_dir(&win.cfg, &win_project.dir_name);
    assert!(std::path::Path::new(path).starts_with(&artifacts), "{path}");
    assert_eq!(std::fs::read(path).unwrap(), shot);
    let description = got["item"]["description"].as_str().unwrap();
    assert_eq!(description, format!("See ![the error]({path})"));

    // Read again, it is the same copy, though the Mac's bytes have changed
    // since: nothing is fetched.
    let home_copy = stored(&b.p.mac, &b.mac_app, &att.id);
    let mut tampered = shot.clone();
    tampered[100] = 0;
    std::fs::write(&home_copy, &tampered).unwrap();
    let again = b.tester.call("item_get", json!({"id": b.item})).await;
    assert_eq!(again["attachments"][0]["path"], path, "{again}");

    // A copy someone changed on the PC is never handed out: fetched again,
    // the Mac's changed bytes don't hash to its record, and nothing is kept.
    std::fs::write(path, screenshot(700 * 1024 + 1)).unwrap();
    let refused = b.tester.call("item_get", json!({"id": b.item})).await;
    let listed = &refused["attachments"][0];
    assert_eq!(listed["path"], "", "{refused}");
    assert!(
        listed["note"].as_str().unwrap().contains("sha256 mismatch"),
        "{refused}"
    );
    assert!(!std::path::Path::new(path).exists());
    let cache = std::path::Path::new(path).parent().unwrap();
    assert_eq!(std::fs::read_dir(cache).unwrap().count(), 0, "no part left");

    // Its own roles still decide on the Mac: an owner-only setting is refused.
    let raw = b
        .tester
        .call_raw(
            "board_update",
            json!({"board_id": rs_id, "version": 99, "structure_policy": "lead_free"}),
        )
        .await;
    assert!(!error_text(&raw).is_empty());
}
