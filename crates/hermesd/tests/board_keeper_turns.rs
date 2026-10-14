//! The board keeper around bots' turns and refusals (0.19.0): a turn that
//! changed a card without moving it asks once; a refused move raises one row
//! naming the rule; a release-gated board gets only its staleness and rows,
//! its flow untouched; and a linked computer never keeps a board it mirrors.

use crate::common;

use chrono::Duration;
use common::cards::fresh;
use common::keeper::{age, gated, long_running, marks, notes, rows, step, up_long};
use hermesd::actor::Actor;
use hermesd::db::MoveTo;
use serde_json::json;

#[tokio::test]
async fn a_turn_that_changed_a_card_and_moved_nothing_asks_once() {
    let (mut f, _) = fresh(&["Builder"]).await;
    let app = f.d.app.clone();
    let bot = f.bot_ids[0].clone();

    // The first turn reads the card, which starts it: it moved, so nothing
    // to ask, though the bot also commented.
    let item = f.item("Buttons").await;
    f.assign(&item.id, Some("Builder")).await;
    f.turn_starts(0).await;
    f.bots[0].call("item_get", json!({"id": item.id})).await;
    f.bots[0]
        .call("item_comment", json!({"id": item.id, "body": "Starting."}))
        .await;
    f.turn_ends(0).await;
    assert!(notes(&app, &bot).is_empty(), "{:?}", notes(&app, &bot));

    // A turn of work that leaves it in Doing: one ask.
    f.turn_starts(0).await;
    f.bots[0]
        .call(
            "item_comment",
            json!({"id": item.id, "body": "Done: artifacts/b.html"}),
        )
        .await;
    f.turn_ends(0).await;
    let asked = notes(&app, &bot);
    assert_eq!(asked.len(), 1, "{asked:?}");
    assert!(
        asked[0].starts_with(&format!(
            "[card {}] Automatic check: your last turn changed {} “Buttons”, and it is still in \
             Doing (in progress).",
            item.id, item.id
        )),
        "{}",
        asked[0]
    );
    assert!(
        asked[0].contains("move it to Review with item_move"),
        "{}",
        asked[0]
    );

    // It answers with a comment, not a move: not asked again.
    f.turn_starts(0).await;
    f.bots[0]
        .call(
            "item_comment",
            json!({"id": item.id, "body": "Waiting on the copy."}),
        )
        .await;
    f.turn_ends(0).await;
    assert_eq!(notes(&app, &bot).len(), 1, "once");

    // Moved on and back: a new column, so a later turn may ask again. A
    // card in Review is the owner's to move.
    f.bot_move(0, &item.id, "review").await;
    f.turn_starts(0).await;
    f.bots[0]
        .call(
            "item_comment",
            json!({"id": item.id, "body": "Ready for you."}),
        )
        .await;
    f.turn_ends(0).await;
    assert_eq!(notes(&app, &bot).len(), 1, "not in Review");
    f.owner_move(&item.id, "doing", None).await;
    f.turn_starts(0).await;
    f.bots[0]
        .call("item_comment", json!({"id": item.id, "body": "Fixing it."}))
        .await;
    f.turn_ends(0).await;
    assert_eq!(notes(&app, &bot).len(), 2);
}

#[tokio::test]
async fn a_refused_move_raises_one_row_naming_its_rule() {
    let (mut f, _) = fresh(&["Builder"]).await;
    let app = f.d.app.clone();
    let k = up_long(&app);
    let id = f.started_card(0, "Buttons").await;
    f.bot_move(0, &id, "review").await;
    // Its own Done is refused: a bot never approves its own work.
    let refused = f.bot_move(0, &id, "done").await;
    assert_eq!(refused["isError"], json!(true), "{refused}");
    step(&app, k);
    assert_eq!(
        rows(&app, &f.project).len(),
        1,
        "only the review row so far"
    );
    age(&app, &id, Duration::minutes(61));
    step(&app, k);
    let raised = rows(&app, &f.project);
    let blocked: Vec<_> = raised
        .iter()
        .filter(|(id, _)| id.ends_with(":blocked"))
        .collect();
    assert_eq!(blocked.len(), 1, "{raised:?}");
    assert!(
        blocked[0].1.starts_with(&format!(
            "{id} can't move to Done: The mover can't be its assignee"
        )),
        "{raised:?}"
    );
    step(&app, k);
    assert_eq!(
        rows(&app, &f.project).len(),
        2,
        "one each, however many passes"
    );
    // The owner moves it: the move no longer stands refused, and both go.
    f.owner_move(&id, "done", None).await;
    step(&app, k);
    assert!(
        rows(&app, &f.project).is_empty(),
        "{:?}",
        rows(&app, &f.project)
    );
    assert!(marks(&app, &f.project, &id).is_empty());
}

#[tokio::test]
async fn a_release_gated_board_gets_only_staleness_and_rows() {
    let mut f = gated(&["Builder"]).await;
    let app = f.d.app.clone();
    let bot = f.bot_ids[0].clone();
    let item = f.item("Login").await;
    f.assign(&item.id, Some("Builder")).await;
    let todo = f.item("Signup").await;
    f.assign(&todo.id, Some("Builder")).await;
    f.owner_move(&item.id, "doing", Some("to test the keeper"))
        .await;
    let before: Vec<_> = app.db.item_events(&item.id).unwrap();
    let k = long_running();

    // Quiet in Doing past the first threshold, an untouched assignment and
    // a turn that changed the card: no note, on a release-gated board.
    for id in [&item.id, &todo.id] {
        age(&app, id, Duration::hours(3));
    }
    step(&app, &k);
    f.turn_starts(0).await;
    f.bots[0]
        .call("item_comment", json!({"id": item.id, "body": "Working."}))
        .await;
    f.turn_ends(0).await;
    assert!(notes(&app, &bot).is_empty(), "{:?}", notes(&app, &bot));

    // Quiet for six hours: stale and one row, still no note.
    age(&app, &item.id, Duration::hours(7));
    step(&app, &k);
    let raised = rows(&app, &f.project);
    assert_eq!(raised.len(), 1, "{raised:?}");
    assert!(
        raised[0].1.contains("is in Doing and nothing has happened"),
        "{raised:?}"
    );
    assert!(f.card(&item.id).await.stale);
    assert!(notes(&app, &bot).is_empty());

    // The keeper moved nothing: the history only grew by the comment.
    let after = app.db.item_events(&item.id).unwrap();
    assert_eq!(after.len(), before.len() + 1, "{after:?}");
    assert_eq!(f.now(&item.id).column_key, "doing");
    assert_eq!(f.now(&todo.id).column_key, "inbox");
}

#[tokio::test]
async fn a_linked_computer_never_keeps_a_board_it_mirrors() {
    let b = common::peer_board::board().await;
    let mac = &b.p.mac.app;
    let win = &b.p.win.app;
    let item = mac.db.get_item(&b.item).unwrap().unwrap();
    let to = MoveTo {
        column: "doing",
        note: Some("for the keeper"),
        wip_override: false,
        first: false,
    };
    mac.db
        .move_item(&item.id, item.version, &to, &Actor::User)
        .unwrap();
    age(mac, &item.id, Duration::hours(30));

    // The PC, which only mirrors the board, keeps nothing and tells nobody.
    step(win, &long_running());
    assert!(win.db.keeper_marks(&b.win_app).unwrap().is_empty());
    assert!(notes(win, &b.tester_id).is_empty());
    assert!(
        rows(win, &b.win_app).is_empty(),
        "{:?}",
        rows(win, &b.win_app)
    );

    // The board's home does: the card went quiet and stale.
    step(mac, &long_running());
    let kept = marks(mac, &b.mac_app, &item.id);
    assert!(kept.contains(&"silent_stale".to_string()), "{kept:?}");
    assert!(kept.contains(&"stale".to_string()), "{kept:?}");
}
