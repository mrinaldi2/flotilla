//! The running cap and its queue (0.23.0): the owner first, then the lead,
//! then card work in the board's order as it queued (an urgent bug at its
//! front), then the rest; a freed slot starts the head.

use flotillad::kanban::model::{ItemType, Priority};
use serde_json::json;

use crate::common::on_demand::{card, dm, queue, state, task, team, until_state, until_team};
use crate::common::WsClient;

#[tokio::test]
async fn a_full_cap_queues_wakes_in_the_owners_order() {
    let (p, mut bots, project) = team(1, &["lead", "a", "b", "c", "d", "e"]).await;
    let id = |name: &str| {
        let at = ["lead", "a", "b", "c", "d", "e"]
            .iter()
            .position(|n| *n == name)
            .unwrap();
        p.ids[at].clone()
    };
    let mut c = WsClient::connect(&p.d).await;
    for name in ["a", "b", "c", "d", "e"] {
        until_state(&p.d, &id(name), &["asleep"]).await;
    }
    let set = c
        .request(json!({"type": "set_config", "max_running_bots": 2}))
        .await;
    assert_eq!(set["config"]["max_running_bots"], 2, "{set}");

    // A task wakes `a` into the second slot, and keeps it awake.
    let work = card(&p.d, &project, ItemType::Task, Priority::P3);
    task(&mut bots[0], "a", &work, "hold this slot for a while").await;
    until_state(&p.d, &id("a"), &["ready"]).await;
    until_team(&mut c, |t| t["running"] == 2).await;

    // Card work queues in the order it came: a later, higher priority card
    // waits behind it.
    let p2 = card(&p.d, &project, ItemType::Task, Priority::P2);
    let p1 = card(&p.d, &project, ItemType::Task, Priority::P1);
    let bug = card(&p.d, &project, ItemType::Bug, Priority::P0);
    let urgent_task = card(&p.d, &project, ItemType::Task, Priority::P0);
    dm(
        &mut bots[0],
        "b",
        "note",
        &format!("[card {p2}] Have a look"),
    )
    .await;
    until_team(&mut c, |t| queue(t) == [id("b")]).await;
    assert_eq!(state(&p.d, &id("b")), "queued");
    dm(
        &mut bots[0],
        "c",
        "note",
        &format!("[card {p1}] Have a look"),
    )
    .await;
    let t = until_team(&mut c, |t| queue(t).len() == 2).await;
    assert_eq!(queue(&t), [id("b"), id("c")], "no jumping ahead: {t}");

    // A P0 task is no urgent bug: it waits too.
    dm(
        &mut bots[0],
        "e",
        "note",
        &format!("[card {urgent_task}] Look"),
    )
    .await;
    let t = until_team(&mut c, |t| queue(t).len() == 3).await;
    assert_eq!(queue(&t), [id("b"), id("c"), id("e")], "{t}");

    // An urgent bug goes to the front of the card work.
    dm(
        &mut bots[0],
        "d",
        "note",
        &format!("[card {bug}] It's broken"),
    )
    .await;
    let t = until_team(&mut c, |t| queue(t).len() == 4).await;
    assert_eq!(queue(&t), [id("d"), id("b"), id("c"), id("e")], "{t}");
    let head = &t["queue"][0];
    assert_eq!(head["reason"], "card");
    assert_eq!(head["item_id"], bug.as_str());
    assert_eq!(head["priority"], "P0");
    assert_eq!(head["urgent"], true);
    assert_eq!(t["queue"][1]["priority"], "P2");
    assert_eq!(t["queue"][1]["urgent"], false);
    assert!(t["queue"][0]["queued_at"].is_string());

    // The owner's wake goes before all card work.
    let ok = c
        .request(json!({"type": "wake_bot", "bot_id": id("e")}))
        .await;
    assert_eq!(ok["type"], "ok", "{ok}");
    let t = until_team(&mut c, |t| t["queue"][0]["reason"] == "owner").await;
    assert_eq!(queue(&t), [id("e"), id("d"), id("b"), id("c")], "{t}");

    // A freed slot starts the head of the queue.
    c.request(json!({"type": "sleep_bot", "bot_id": id("a")}))
        .await;
    until_state(&p.d, &id("a"), &["asleep"]).await;
    until_state(&p.d, &id("e"), &["starting", "ready"]).await;
    let t = until_team(&mut c, |t| queue(t).len() == 3).await;
    assert_eq!(queue(&t), [id("d"), id("b"), id("c")], "{t}");
    assert_eq!(t["running"], 2);

    // A queued bot sent to sleep leaves the queue.
    c.request(json!({"type": "sleep_bot", "bot_id": id("c")}))
        .await;
    let t = until_team(&mut c, |t| queue(t).len() == 2).await;
    assert_eq!(queue(&t), [id("d"), id("b")], "{t}");
    assert_eq!(state(&p.d, &id("c")), "asleep");
}

/// The time a bot waits for a slot isn't counted against its task's
/// deadline.
#[tokio::test]
async fn a_tasks_deadline_doesnt_count_time_queued() {
    let (p, mut bots, project) = team(1, &["lead", "a", "b"]).await;
    let (a, b) = (p.ids[1].clone(), p.ids[2].clone());
    let mut c = WsClient::connect(&p.d).await;
    until_state(&p.d, &a, &["asleep"]).await;
    until_state(&p.d, &b, &["asleep"]).await;
    c.request(json!({"type": "set_config", "max_running_bots": 2}))
        .await;
    let work = card(&p.d, &project, ItemType::Task, Priority::P3);
    task(&mut bots[0], "a", &work, "hold this slot").await;
    until_state(&p.d, &a, &["ready"]).await;

    let later = card(&p.d, &project, ItemType::Task, Priority::P3);
    task(&mut bots[0], "b", &later, "your turn will come").await;
    until_team(&mut c, |t| queue(t) == [b.clone()]).await;
    let task = p.d.app.db.open_tasks_for(&b).unwrap();
    let before = task[0].deadline_at.expect("a deadline");
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    c.request(json!({"type": "sleep_bot", "bot_id": a})).await;
    until_state(&p.d, &b, &["starting", "ready"]).await;
    let after = p.d.app.db.open_tasks_for(&b).unwrap()[0]
        .deadline_at
        .unwrap();
    assert!(
        after - before >= chrono::Duration::seconds(2),
        "{before} → {after}"
    );
}
