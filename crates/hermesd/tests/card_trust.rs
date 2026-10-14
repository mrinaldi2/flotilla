//! What a bot can't do with the rights its own card gives it (0.19.0 card
//! flow, review of PR #29): approve its own work by handing the card on
//! first, start cards past the Doing column's limit or rules, restart a
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
    let item = f.item("Pricing table").await;
    f.assign(&item.id, Some("Builder")).await;
    let got = f.bots[0].call("item_get", json!({"id": item.id})).await;
    assert_eq!(got["item"]["column_key"], "doing", "{got}");
    let review = f.bot_move(0, &item.id, "review").await;
    assert_ne!(review["isError"], json!(true), "{review}");

    // Taken off itself, the lead still did the work.
    bot_assign(&mut f, 0, &item.id, None).await;
    let unassigned = f.bot_move(0, &item.id, "done").await;
    assert!(
        error_text(&unassigned).contains("rule.independent"),
        "{unassigned}"
    );
    // Nor when it hands the card to another bot.
    bot_assign(&mut f, 0, &item.id, Some("Helper")).await;
    let handed = f.bot_move(0, &item.id, "done").await;
    assert!(error_text(&handed).contains("rule.independent"), "{handed}");
    assert_eq!(f.now(&item.id).column_key, "review");

    // A lead who never held it may: that is the review.
    let other = f.item("Footer").await;
    f.assign(&other.id, Some("Helper")).await;
    f.bots[1].call("item_get", json!({"id": other.id})).await;
    let r = f.bot_move(1, &other.id, "review").await;
    assert_ne!(r["isError"], json!(true), "{r}");
    let approved = f.bot_move(0, &other.id, "done").await;
    assert_ne!(approved["isError"], json!(true), "{approved}");
}
