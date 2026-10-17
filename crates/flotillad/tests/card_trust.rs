//! What a bot can't do with the rights its own card gives it (0.19.0 card
//! flow, review of PR #29): approve its own work by handing the card on
//! first, take cards into Doing past its limit or rules, restart a
//! card the owner moved back, or block another card from its own.

use crate::common;

use common::cards::fresh;
use common::tasks::error_text;
use serde_json::json;

/// Bot `i` assigns the item to `bot` (or takes it off) with `item_assign`.
async fn bot_assign(f: &mut common::cards::Fresh, i: usize, id: &str, bot: Option<&str>) {
    let version = f.now(id).version;
    let reply = f.bots[i]
        .call_raw(
            "item_assign",
            json!({"id": id, "expected_version": version, "bot": bot}),
        )
        .await;
    assert_ne!(reply["isError"], json!(true), "{reply}");
}

#[tokio::test]
async fn the_lead_cant_approve_its_own_card_by_handing_it_on() {
    let (mut f, _) = fresh(&["Builder", "Helper"]).await;
    let id = f.started_card(0, "Pricing table").await;
    let review = f.bot_move(0, &id, "review").await;
    assert_ne!(review["isError"], json!(true), "{review}");

    // Taken off itself, the lead still did the work.
    bot_assign(&mut f, 0, &id, None).await;
    let unassigned = f.bot_move(0, &id, "done").await;
    assert!(
        error_text(&unassigned).contains("rule.independent"),
        "{unassigned}"
    );
    // Nor when it hands the card to another bot.
    bot_assign(&mut f, 0, &id, Some("Helper")).await;
    let handed = f.bot_move(0, &id, "done").await;
    assert!(error_text(&handed).contains("rule.independent"), "{handed}");
    assert_eq!(f.now(&id).column_key, "review");

    // A lead who never held it may: that is the review.
    let other = f.started_card(1, "Footer").await;
    let r = f.bot_move(1, &other, "review").await;
    assert_ne!(r["isError"], json!(true), "{r}");
    let approved = f.bot_move(0, &other, "done").await;
    assert_ne!(approved["isError"], json!(true), "{approved}");
}

#[tokio::test]
async fn a_read_never_starts_a_card_past_the_doing_limit() {
    let (mut f, _) = fresh(&["Builder"]).await;
    let mut ids = Vec::new();
    for title in ["One", "Two", "Three", "Four"] {
        let item = f.item(title).await;
        f.assign(&item.id, Some("Builder")).await;
        ids.push(item.id);
    }
    for id in &ids {
        let got = f.bots[0].call("item_get", json!({"id": id})).await;
        assert_eq!(got["item"]["column_key"], "research", "{got}");
        f.research(0, id).await;
    }
    for id in &ids[..3] {
        let moved = f.bot_move(0, id, "doing").await;
        assert_ne!(moved["isError"], json!(true), "{moved}");
    }
    // Doing holds 3: the fourth stays in Research, and the refusal says why.
    let fourth = f.bot_move(0, &ids[3], "doing").await;
    assert_eq!(f.now(&ids[3]).column_key, "research");
    let why = error_text(&fourth);
    assert!(why.contains("Doing is at its WIP limit of 3"), "{why}");
}

#[tokio::test]
async fn a_card_the_owner_moved_back_isnt_started_again_by_a_read() {
    let (mut f, _) = fresh(&["Builder"]).await;
    let item = f.item("Search box").await;
    f.assign(&item.id, Some("Builder")).await;
    let got = f.bots[0].call("item_get", json!({"id": item.id})).await;
    assert_eq!(got["item"]["column_key"], "research", "{got}");

    // The owner says "not now": back to To do. The bot reads the notice
    // and the card; the card stays where the owner put it.
    let back = f.owner_move(&item.id, "todo", None).await;
    assert!(back.outcome.is_some());
    let read = f.bots[0].call("item_get", json!({"id": item.id})).await;
    assert_eq!(read["item"]["column_key"], "todo", "{read}");
    assert!(read.get("started").is_none(), "{read}");
    assert_eq!(f.now(&item.id).column_key, "todo");
}

#[tokio::test]
async fn an_assignee_without_a_role_cant_block_another_card() {
    let (mut f, _) = fresh(&["Lead", "Helper"]).await;
    let mine = f.item("Login form").await;
    let other = f.item("Signup form").await;
    f.assign(&mine.id, Some("Helper")).await;
    let blocks = f.bots[1]
        .call_raw(
            "item_link",
            json!({"id": mine.id, "kind": "item:blocks", "ref": other.id}),
        )
        .await;
    assert!(error_text(&blocks).contains("board role"), "{blocks}");
    assert!(f
        .d
        .app
        .db
        .board_read(|t| t.blockers_of(&other.id))
        .unwrap()
        .is_empty());
    // Saying the cards are related changes nothing about the other card.
    let relates = f.bots[1]
        .call_raw(
            "item_link",
            json!({"id": mine.id, "kind": "item:relates", "ref": other.id}),
        )
        .await;
    assert_ne!(relates["isError"], json!(true), "{relates}");
}
