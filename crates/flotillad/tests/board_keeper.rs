//! The board keeper on a Simple + Review board (0.19.0), on the test's
//! clock: a card quiet in Doing (past Research, 0.21.0) is asked about once, then flagged stale with
//! one Needs-you row; an untouched assignment is told again once; a card in
//! Review waits for the owner; each resets when the card changes, survives
//! a restart without a second note, and a long downtime brings no storm.

use crate::common;

use chrono::Duration;
use common::cards::fresh;
use common::keeper::{age, long_running, marks, notes, rows, started, step, up_long};
use serde_json::json;

#[tokio::test]
async fn a_quiet_card_in_doing_is_asked_about_once_then_flagged_once() {
    let (mut f, _) = fresh(&["Builder"]).await;
    let app = f.d.app.clone();
    let bot = f.bot_ids[0].clone();
    let id = f.started_card(0, "Buttons").await;
    let k = &long_running();

    // Not yet quiet for 2 h: nothing.
    age(&app, &id, Duration::minutes(110));
    step(&app, k);
    assert!(notes(&app, &bot).is_empty());

    // Two hours: one note, from the daemon, about this card.
    age(&app, &id, Duration::minutes(15));
    step(&app, k);
    let told = notes(&app, &bot);
    assert_eq!(told.len(), 1, "{told:?}");
    let head =
        format!("[card {id}] Automatic check: {id} “Buttons” is assigned to you and is in Doing");
    for part in [
        head.as_str(),
        "nothing has happened on it for 2 h",
        "move it to Review with item_move",
        "move it back to To do and ask the owner",
        "not the owner",
    ] {
        assert!(told[0].contains(part), "missing {part:?} in {}", told[0]);
    }
    // Sent as a note from `system`, the daemon: never as the owner.
    let conv = app.db.dm_conversation(&bot).unwrap().unwrap();
    let msg = app.db.list_messages(&conv.id, None, 50).unwrap();
    let note = msg.iter().find(|m| m.body == told[0]).unwrap();
    assert_eq!(note.kind, bus::MessageKind::Note);
    assert_eq!(note.sender.name, "system");

    // Not again, however long; no row before 6 h.
    age(&app, &id, Duration::hours(3));
    step(&app, k);
    assert_eq!(notes(&app, &bot).len(), 1);
    assert!(rows(&app, &f.project).is_empty());
    assert!(!f.card(&id).await.stale);

    // Six hours quiet: stale, and one row for the owner.
    age(&app, &id, Duration::hours(1));
    step(&app, k);
    assert!(f.card(&id).await.stale, "the card shows stale");
    let raised = rows(&app, &f.project);
    assert_eq!(raised.len(), 1, "{raised:?}");
    assert!(
        raised[0].1.starts_with(&format!(
            "{id} is in Doing and nothing has happened for 6 h"
        )),
        "{raised:?}"
    );
    age(&app, &id, Duration::hours(10));
    step(&app, k);
    step(&app, k);
    assert_eq!(notes(&app, &bot).len(), 1, "still one note");
    assert_eq!(rows(&app, &f.project).len(), 1, "still one row");

    // The bot comments: the card changed, so the flag and the row go, and
    // the timer starts again.
    f.bots[0]
        .call("item_comment", json!({"id": id, "body": "Still on it."}))
        .await;
    step(&app, k);
    assert!(!f.card(&id).await.stale);
    assert!(rows(&app, &f.project).is_empty());
    assert!(
        marks(&app, &f.project, &id).is_empty(),
        "{:?}",
        marks(&app, &f.project, &id)
    );
    age(&app, &id, Duration::minutes(125));
    step(&app, k);
    assert_eq!(notes(&app, &bot).len(), 2, "a new quiet spell, a new note");
}

#[tokio::test]
async fn a_restart_sends_nothing_again_and_a_downtime_brings_no_storm() {
    let (mut f, _) = fresh(&["Builder"]).await;
    let app = f.d.app.clone();
    let bot = f.bot_ids[0].clone();
    let id = f.started_card(0, "Buttons").await;
    age(&app, &id, Duration::hours(3));
    step(&app, &long_running());
    assert_eq!(notes(&app, &bot).len(), 1);

    // A new daemon (fresh memory) finds the mark: no second note.
    let restarted = long_running();
    step(&app, &restarted);
    assert_eq!(notes(&app, &bot).len(), 1);

    // Many cards gone quiet while the daemon was down: right after it
    // starts, nothing; once their time has passed since the start, one note
    // per bot and pass, not one per card.
    let mut todo = Vec::new();
    for n in 0..4 {
        let item = f.item(&format!("Page {n}")).await;
        f.assign(&item.id, Some("Builder")).await;
        age(&app, &item.id, Duration::days(3));
        todo.push(item.id);
    }
    let just_up = started(Duration::minutes(10));
    step(&app, &just_up);
    assert_eq!(
        notes(&app, &bot).len(),
        1,
        "nothing at once after a downtime"
    );
    let up_a_while = started(Duration::minutes(31));
    step(&app, &up_a_while);
    let after_one = notes(&app, &bot);
    assert_eq!(after_one.len(), 2, "one reminder this pass: {after_one:?}");
    step(&app, &up_a_while);
    assert_eq!(notes(&app, &bot).len(), 3, "and one the next");
}

#[tokio::test]
async fn an_untouched_assignment_is_told_again_once() {
    let (mut f, _) = fresh(&["Builder"]).await;
    let app = f.d.app.clone();
    let bot = f.bot_ids[0].clone();
    let item = f.item("Checkout").await;
    f.assign(&item.id, Some("Builder")).await;
    f.heard(0, "assigned").await;
    let k = up_long(&app);
    age(&app, &item.id, Duration::minutes(20));
    step(&app, k);
    assert!(notes(&app, &bot).is_empty());

    age(&app, &item.id, Duration::minutes(15));
    step(&app, k);
    let told = notes(&app, &bot);
    assert_eq!(told.len(), 1, "{told:?}");
    let reminder = &told[0];
    assert!(
        reminder.starts_with(&format!(
            "[card {}] Automatic reminder: {} “Checkout” is assigned to you and is still in To do",
            item.id, item.id
        )),
        "{reminder}"
    );
    for part in [
        "Start now: read it with item_get, which moves it to Research.",
        "write what you found under “Code research”",
        "then move it to Doing.",
    ] {
        assert!(reminder.contains(part), "missing {part:?} in {reminder}");
    }
    age(&app, &item.id, Duration::hours(5));
    step(&app, k);
    assert_eq!(notes(&app, &bot).len(), 1, "once");

    // Taken off and given back: a new assignment, a new reminder later.
    f.assign(&item.id, None).await;
    f.assign(&item.id, Some("Builder")).await;
    step(&app, k);
    assert_eq!(notes(&app, &bot).len(), 1, "not before its time");
    age(&app, &item.id, Duration::minutes(31));
    step(&app, k);
    assert_eq!(notes(&app, &bot).len(), 2);

    // A bot that read its card has touched it: nothing to remind.
    let other = f.item("Footer").await;
    f.assign(&other.id, Some("Builder")).await;
    // Moved since the assignment, so the bot's read doesn't start it.
    f.owner_move(&other.id, "research", None).await;
    f.owner_move(&other.id, "todo", None).await;
    let read = f.bots[0].call("item_get", json!({"id": other.id})).await;
    assert_eq!(read["item"]["column_key"], "todo", "{read}");
    age(&app, &other.id, Duration::hours(2));
    step(&app, k);
    let about_other: Vec<_> = notes(&app, &bot)
        .into_iter()
        .filter(|n| n.contains(&other.id))
        .collect();
    assert!(about_other.is_empty(), "{about_other:?}");
}

#[tokio::test]
async fn a_card_in_review_waits_for_the_owner_until_it_leaves() {
    let (mut f, _) = fresh(&["Builder"]).await;
    let app = f.d.app.clone();
    let id = f.started_card(0, "Buttons").await;
    let moved = f.bot_move(0, &id, "review").await;
    assert_ne!(moved["isError"], json!(true), "{moved}");
    let waiting = rows(&app, &f.project);
    assert_eq!(waiting.len(), 1, "{waiting:?}");
    assert_eq!(waiting[0].1, format!("{id} waits for your review: Buttons"));
    // One row however often it is read or passed over.
    step(&app, &long_running());
    assert_eq!(rows(&app, &f.project).len(), 1);
    // Nobody is nudged about a card that waits for the owner.
    age(&app, &id, Duration::hours(10));
    step(&app, &long_running());
    assert!(notes(&app, &f.bot_ids[0]).is_empty());
    f.owner_move(&id, "done", None).await;
    assert!(
        rows(&app, &f.project).is_empty(),
        "gone once it left Review"
    );
}

#[tokio::test]
async fn a_card_past_the_boards_stale_time_is_flagged_and_pushed() {
    let (mut f, _) = fresh(&["Builder"]).await;
    let app = f.d.app.clone();
    let id = f.started_card(0, "Buttons").await;
    // A comment an hour ago keeps the bot from being quiet; the column's
    // age is what counts.
    let mut feed = app.board.subscribe();
    age(&app, &id, Duration::hours(25));
    f.bots[0]
        .call("item_comment", json!({"id": id, "body": "Halfway."}))
        .await;
    while feed.try_recv().is_ok() {}
    step(&app, &long_running());
    let pushed = feed.try_recv().expect("the card is pushed");
    assert_eq!(pushed.item_id, id);
    assert!(pushed.card.as_ref().is_some_and(|c| c.stale), "{pushed:?}");
    assert_eq!(
        pushed.actor.as_deref(),
        Some("daemon"),
        "the daemon's change"
    );
    assert!(f.card(&id).await.stale);
    // Once only.
    step(&app, &long_running());
    assert!(feed.try_recv().is_err());
    // It moves on: no longer stale.
    f.bot_move(0, &id, "review").await;
    assert!(!f.card(&id).await.stale);
}

#[tokio::test]
async fn the_owner_hears_what_the_keeper_did_as_automatic() {
    use bus::contract::home::BoardActivityKind;
    use flotillad::events::Push;

    let (mut f, _) = fresh(&["Builder"]).await;
    let app = f.d.app.clone();
    let id = f.started_card(0, "Buttons").await;
    let mut pushes = app.events.subscribe_push();
    let mut checked = || {
        let mut out = Vec::new();
        while let Ok(push) = pushes.try_recv() {
            if let Push::BoardActivity { activity, .. } = push {
                if activity.kind == BoardActivityKind::Checked as i32 {
                    out.push(*activity);
                }
            }
        }
        out
    };
    age(&app, &id, Duration::hours(2));
    step(&app, &long_running());
    let asked = checked();
    assert_eq!(asked.len(), 1, "{asked:?}");
    assert_eq!(asked[0].actor, "daemon");
    assert_eq!(asked[0].item_id, id);
    assert_eq!(asked[0].title, format!("Asked Builder about {id}"));
    assert!(
        asked[0]
            .body
            .ends_with("In Doing, nothing has happened for 2 h"),
        "{}",
        asked[0].body
    );
    age(&app, &id, Duration::hours(4));
    step(&app, &long_running());
    let raised = checked();
    assert_eq!(raised.len(), 1, "{raised:?}");
    assert_eq!(raised[0].title, format!("{id} needs you"));
    assert!(raised[0].assignee.is_empty());
    assert!(
        raised[0].body.contains("nothing has happened for 6 h"),
        "{}",
        raised[0].body
    );
    step(&app, &long_running());
    assert!(checked().is_empty(), "said once");
}
