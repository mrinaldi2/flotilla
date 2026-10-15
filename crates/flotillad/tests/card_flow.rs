//! How a card flows between the owner and a bot on a new project (0.19.0
//! card flow, the owner's test of the installed build): the first board is
//! Simple + Review, the first bot leads, an assignment tells the bot what
//! to do, its first read moves the card to Doing, it moves the card to
//! Review and the owner moves it to Done. A card that isn't ready goes back
//! to To do with a question, and the bot's reads no longer start it.

use crate::common;

use common::cards::{column_names, fresh};
use common::tasks::error_text;
use serde_json::json;

#[tokio::test]
async fn a_simple_project_flows_from_to_do_to_done() {
    // A new project and its first bot, which leads it; the owner's first
    // look at the board starts Simple + Review.
    let (mut f, s) = fresh(&["Builder"]).await;
    let bot_id = f.bot_ids[0].clone();
    let project =
        f.d.app
            .db
            .get_project(&f.project)
            .unwrap()
            .expect("project");
    assert_eq!(project.lead_bot_id.as_deref(), Some(bot_id.as_str()));
    assert_eq!(
        column_names(&s),
        ["To do", "Doing", "Review", "Done", "Cancelled"]
    );
    let main = s.boards.iter().find(|b| b.is_default).expect("Main");
    assert_eq!((main.name.as_str(), main.flow.as_str()), ("Main", "custom"));
    assert_eq!(main.automations["work.started"], "doing");

    // The owner creates a card and assigns it: the bot is told which card,
    // where it is and what to do.
    let item = f.item("Button variants").await;
    assert_eq!(item.column_key, "todo");
    f.assign(&item.id, Some("Builder")).await;
    let told = f.heard(0, "assigned").await;
    for part in [
        &format!(
            "[card {}] The owner assigned {} “Button variants” to you.",
            item.id, item.id
        ),
        "It is in To do (not started).",
        "Start now: read it with item_get, which moves it to Doing.",
        "move it to Review with item_move",
        "after moving it back to To do",
        &format!("message_owner (asks: true, item: {})", item.id),
    ] {
        assert!(told.contains(part), "missing {part:?} in {told}");
    }

    // The bot reads it: the daemon moves it to Doing, as the fact.
    let got = f.bots[0].call("item_get", json!({"id": item.id})).await;
    assert_eq!(got["item"]["column_key"], "doing", "{got}");
    assert!(
        got["started"]
            .as_str()
            .unwrap_or_default()
            .contains("moved to Doing"),
        "{got}"
    );
    let history = f.d.app.db.item_events(&item.id).unwrap();
    let start = history.last().expect("the move");
    assert_eq!(start.actor, "daemon");
    assert_eq!(start.to.as_deref(), Some("doing"));
    assert!(
        start
            .note
            .as_deref()
            .is_some_and(|n| n.starts_with("work.started")),
        "{start:?}"
    );
    // Reading it again changes nothing.
    let again = f.bots[0].call("item_get", json!({"id": item.id})).await;
    assert!(again.get("started").is_none(), "{again}");

    // It finishes: a comment, a link, and Review. Done isn't its move.
    f.bots[0]
        .call(
            "item_comment",
            json!({"id": item.id, "body": "Done: artifacts/buttons.html"}),
        )
        .await;
    let review = f.bot_move(0, &item.id, "review").await;
    assert_ne!(review["isError"], json!(true), "{review}");
    assert_eq!(f.now(&item.id).column_key, "review");
    let own_done = f.bot_move(0, &item.id, "done").await;
    assert!(
        error_text(&own_done).contains("rule.independent"),
        "{own_done}"
    );

    // The owner checks it and moves it to Done; the bot hears there is
    // nothing left to do.
    let done = f.owner_move(&item.id, "done", None).await;
    assert!(
        matches!(
            done.outcome,
            Some(bus::contract::board::move_result::Outcome::Done(_))
        ),
        "{done:?}"
    );
    let told = f.heard(0, "moved it to Done").await;
    assert!(
        told.contains("It is done: there is nothing to do."),
        "{told}"
    );
    assert!(
        told.contains("Your last comment on it: “Done: artifacts/buttons.html”"),
        "{told}"
    );
}

#[tokio::test]
async fn a_card_that_isnt_ready_goes_back_to_to_do_with_a_question() {
    let (mut f, _) = fresh(&["Builder"]).await;
    let item = f.item("Checkout page").await;
    f.assign(&item.id, Some("Builder")).await;
    f.heard(0, "assigned").await;
    let got = f.bots[0].call("item_get", json!({"id": item.id})).await;
    assert_eq!(got["item"]["column_key"], "doing");

    // Not ready: back to To do, and a question for the owner on the card.
    let back = f.bot_move(0, &item.id, "todo").await;
    assert_ne!(back["isError"], json!(true), "{back}");
    let asked = f.bots[0]
        .call(
            "message_owner",
            json!({"body": "Which payment providers?", "asks": true, "item": item.id}),
        )
        .await;
    assert_eq!(asked["asks"], true, "{asked}");
    let open = hermesd::owner_threads::open_questions(&f.d.app, Some(&f.project), None).unwrap();
    assert_eq!(open.len(), 1, "{open:?}");

    // Its next reads don't start it again: it moves it on itself once it
    // has its answer.
    let read = f.bots[0].call("item_get", json!({"id": item.id})).await;
    assert_eq!(read["item"]["column_key"], "todo", "{read}");
    assert!(read.get("started").is_none(), "{read}");
    let version = f.now(&item.id).version;
    let edited = f.bots[0]
        .call(
            "item_update",
            json!({"id": item.id, "expected_version": version,
                   "description": "Stripe and PayPal."}),
        )
        .await;
    assert!(edited.get("started").is_none(), "{edited}");
    assert_eq!(f.now(&item.id).column_key, "todo");
    let start = f.bot_move(0, &item.id, "doing").await;
    assert_ne!(start["isError"], json!(true), "{start}");

    // Assigned again, it starts on the next read once more.
    f.assign(&item.id, None).await;
    let back = f.owner_move(&item.id, "todo", None).await;
    assert!(back.outcome.is_some());
    f.assign(&item.id, Some("Builder")).await;
    let read = f.bots[0].call("item_get", json!({"id": item.id})).await;
    assert_eq!(read["item"]["column_key"], "doing", "{read}");
}

#[tokio::test]
async fn a_release_gated_card_is_never_moved_for_its_assignee() {
    let d = common::spawn_daemon().await;
    let mut owner = common::WsClient::connect(&d).await;
    let project = common::peers::project(&mut owner, "Old Shop").await;
    // An existing project's board, from before the first-board preset.
    d.app
        .db
        .ensure_board(&project, &d.app.db.daemon_id().unwrap(), Some("OS"))
        .unwrap();
    let bot = common::create_bot(&mut owner, &project, "Builder").await;
    let bot_id = bot["id"].as_str().expect("id").to_string();
    let token = d.app.secrets.bot_token(&bot_id).expect("token");
    let mut builder = common::McpClient::new(&d, &token);
    let (id, version) = common::board::new_item(
        &d.app.db,
        &project,
        "Legacy",
        hermesd::kanban::model::Priority::P2,
    );
    d.app
        .db
        .assign_item(&id, version, Some(&bot_id), &hermesd::actor::Actor::User)
        .unwrap();
    let got = builder.call("item_get", json!({"id": id})).await;
    assert_eq!(got["item"]["column_key"], "inbox", "{got}");
    assert!(got.get("started").is_none(), "{got}");
}
