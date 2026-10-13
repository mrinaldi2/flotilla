//! Steps 5 to 7 of the K9 scenario: the cook, on the linked PC, takes the
//! item through To do → Doing → QA → Done, refused once with the rule it
//! misses; the phone approves the lead's second board; and with the home
//! offline the cook's reads still work and its writes name the home.

use bus::contract::board::{self as c, board_request::Request};
use serde_json::{json, Value};

use super::super::board::call;
use super::super::peers::wait_until;
use super::{approve, board, column, proposed, refusal, version_of, K9};

/// The cook moves `id` to `to`, reading its version first.
async fn cook_moves(k: &mut K9, id: &str, to: &str) -> Value {
    let got = k.cook.call("item_get", json!({ "id": id })).await;
    k.cook
        .call_raw(
            "item_move",
            json!({"id": id, "to": to, "expected_version": version_of(&got["item"])}),
        )
        .await
}

/// The lead ticks every criterion, with what shows it holds.
async fn lead_checks(k: &mut K9, id: &str) {
    for index in 0..2 {
        let got = k.lead.call("item_get", json!({ "id": id })).await;
        k.lead
            .call(
                "item_check_ac",
                json!({"id": id, "expected_version": version_of(&got["item"]),
                       "index": index, "result": "pass",
                       "evidence": "Cooked at 1x and 2x; photos on the card."}),
            )
            .await;
    }
}

pub async fn step5(k: &mut K9, s: &c::BoardSnapshot, recipes: &str, id: &str) {
    let (doing, qa, done) = (
        column(s, recipes, "Doing"),
        column(s, recipes, "QA"),
        column(s, recipes, "Done"),
    );
    let raw = cook_moves(k, id, &doing).await;
    assert_ne!(raw["isError"], json!(true), "To do → Doing: {raw}");
    let raw = cook_moves(k, id, &qa).await;
    let text = refusal(&raw);
    assert!(text.contains("rule.ac_all_checked"), "{text}");
    assert!(
        text.contains("2 acceptance criteria are not checked"),
        "{text}"
    );
    lead_checks(k, id).await;
    for to in [&qa, &done] {
        let raw = cook_moves(k, id, to).await;
        assert_ne!(raw["isError"], json!(true), "to {to}: {raw}");
    }
    let card = k
        .desktop_board()
        .await
        .cards
        .into_iter()
        .find(|card| card.id == id)
        .expect("the card");
    assert_eq!(card.column_key, done);
}

/// Returns the Research board's id.
pub async fn step6(k: &mut K9) -> String {
    let reply = k
        .lead
        .call(
            "board_create",
            json!({"name": "Research", "key": "RS", "preset": "simple"}),
        )
        .await;
    let proposal = proposed(&reply);
    let get = Request::BoardGet(c::BoardGet {
        project_id: k.mac_rb.clone(),
    });
    let seen = super::super::board::snapshot(call(&mut k.phone, get).await);
    let open = seen.proposals.iter().find(|p| p.id == proposal);
    assert_eq!(open.map(|p| p.state.as_str()), Some("open"), "{seen:?}");
    let s = approve(&mut k.phone, &proposal).await;
    let research = board(&s, "Research").id.clone();
    let ruled = k.p.mac.app.db.board_proposal(&proposal).unwrap();
    assert_eq!(ruled.expect("proposal").state, "applied");
    let listed = k.lead.call("board_list", json!({})).await;
    let names: Vec<&str> = listed["boards"]
        .as_array()
        .expect("boards")
        .iter()
        .filter_map(|b| b["board"]["name"].as_str())
        .collect();
    assert_eq!(names, ["Recipes", "Research"], "{listed}");
    research
}

/// Returns the item it moved.
pub async fn step7(k: &mut K9, research: &str) -> String {
    let s = k.desktop_board().await;
    let doing = column(&s, research, "Doing");
    let created = k
        .lead
        .call(
            "item_create",
            json!({"board_id": research, "type": "spike", "title": "Compare flours"}),
        )
        .await;
    let id = created["item"]["id"].as_str().expect("id").to_string();
    assert!(id.starts_with("RS-"), "{created}");
    let got = k.lead.call("item_get", json!({ "id": id })).await;
    k.lead
        .call(
            "item_assign",
            json!({"id": id, "expected_version": version_of(&got["item"]), "bot": "Cook"}),
        )
        .await;
    let raw = cook_moves(k, &id, &doing).await;
    assert_ne!(raw["isError"], json!(true), "{raw}");
    k.pc_mirrors("the PC mirrors the move", |m| {
        m.cards
            .iter()
            .any(|card| card.id == id && card.column_key == doing)
    })
    .await;

    // The home goes offline: the Mac forgets the PC, so nothing redials.
    let mac_peer = k.p.mac_peer_id.clone();
    k.p.mac.app.db.revoke_peer(&mac_peer).unwrap();
    k.p.mac.app.peers.disconnect(&mac_peer);
    let (win, win_peer) = (&k.p.win.app, k.p.win_peer_id.clone());
    wait_until("the PC loses the Mac", || !win.peers.is_online(&win_peer)).await;

    let got = k.cook.call("board_get", json!({})).await;
    let offline = got["offline"].as_str().unwrap_or_default();
    assert!(offline.contains("read-only"), "{got}");
    let cards = got["cards"].as_array().expect("cards");
    assert!(cards.iter().any(|card| card["id"] == id), "{got}");
    // As on the home: the default board's key, no archived board.
    assert_eq!(got["key"], "RC", "{got}");
    let columns = got["columns"].as_array().expect("columns");
    assert!(columns.iter().all(|col| col["key"] != "deploying"), "{got}");
    let found = k.cook.call("item_query", json!({"text": "flours"})).await;
    assert_eq!(found["cards"].as_array().map(Vec::len), Some(1), "{found}");
    let listed = k.cook.call("board_list", json!({})).await;
    let boards = listed["boards"].as_array();
    assert!(boards.is_some_and(|b| !b.is_empty()), "{listed}");

    let todo = column(&s, research, "To do");
    let raw = k
        .cook
        .call_raw(
            "item_move",
            json!({"id": id, "to": todo, "expected_version": 1}),
        )
        .await;
    assert_eq!(
        refusal(&raw),
        "The board lives on mac, which is unreachable. Try again when it's back."
    );
    id
}
