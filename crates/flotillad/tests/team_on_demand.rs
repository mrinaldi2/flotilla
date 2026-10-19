//! The team on demand (0.23.0): presence, sleeping when idle, and waking on
//! a message, a routine or the owner.

use bus::BotPresence;
use flotillad::db::Db;
use serde_json::json;

use crate::common::on_demand::{daemon, dm, state, until_state, until_team};
use crate::common::tasks::project_with_bots_on;
use crate::common::WsClient;

/// Upgrading keeps each project's lead on and makes every other bot on
/// demand; a bot made lead later is always on, and a new bot is on demand.
#[test]
fn upgrading_makes_every_bot_but_the_lead_on_demand() {
    let home = tempfile::tempdir().unwrap();
    let path = home.path().join("bus.sqlite");
    let db = Db::open(&path).unwrap();
    let project = db.create_project("p", "p").unwrap().id;
    let bot = |name: &str| {
        let workspace = home.path().join(name).display().to_string();
        db.create_bot(&project, name, "", "", "", &workspace, name, None)
            .unwrap()
            .id
    };
    let (lead, dev, qa) = (bot("lead"), bot("dev"), bot("qa"));
    db.set_project_lead(&project, Some(&lead)).unwrap();
    db.set_bot_presence(&dev, BotPresence::AlwaysOn).unwrap();
    drop(db);
    // Back to a database from before the migration: no presence at all.
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute_batch(
        "DROP TRIGGER project_lead_always_on;
         ALTER TABLE bot DROP COLUMN presence;
         UPDATE meta SET value = '76' WHERE key = 'schema_version';",
    )
    .unwrap();
    drop(conn);

    let db = Db::open(&path).unwrap();
    let presence = |id: &str| db.get_bot(id).unwrap().unwrap().presence;
    assert_eq!(presence(&lead), BotPresence::AlwaysOn);
    assert_eq!(presence(&dev), BotPresence::OnDemand);
    assert_eq!(presence(&qa), BotPresence::OnDemand);

    db.set_project_lead(&project, Some(&qa)).unwrap();
    assert_eq!(presence(&qa), BotPresence::AlwaysOn);
    let workspace = home.path().join("new").display().to_string();
    let new = db
        .create_bot(&project, "new", "", "", "", &workspace, "new", None)
        .unwrap();
    assert_eq!(new.presence, BotPresence::OnDemand);
}

/// An on-demand bot with nothing to do sleeps after the idle time, giving
/// its slot back; a message to it wakes it, and is delivered once it is up.
#[tokio::test]
async fn an_idle_bot_sleeps_and_a_message_wakes_it() {
    let (p, mut bots) = project_with_bots_on(daemon(1).await, &["lead", "dev"]).await;
    let (lead, dev) = (p.ids[0].clone(), p.ids[1].clone());
    let mut c = WsClient::connect(&p.d).await;
    let presence = |id: &str| p.d.app.db.get_bot(id).unwrap().unwrap().presence;
    assert_eq!(presence(&lead), BotPresence::AlwaysOn);
    assert_eq!(presence(&dev), BotPresence::OnDemand);

    until_state(&p.d, &dev, &["asleep"]).await;
    let team = until_team(&mut c, |t| t["running"] == 1).await;
    assert_eq!(team["max_running"], 6);
    assert_eq!(team["idle_sleep_minutes"], 10);
    assert_ne!(state(&p.d, &lead), "asleep", "the lead is always on");
    // Asleep is no failure: the Board glance shows it asleep.
    let listed = c
        .request(json!({"type": "list_bots", "project_id": p.d.app.db.get_bot(&dev).unwrap().unwrap().project_id}))
        .await;
    let row = listed["bots"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["id"] == dev.as_str())
        .cloned()
        .unwrap();
    assert_eq!(row["state"], "asleep", "{row}");
    assert_eq!(row["presence"], "on_demand", "{row}");

    dm(&mut bots[0], "dev", "note", "wake up, please").await;
    let pushed = c
        .wait_for(|v| v["type"] == "team_updated" && v["team"]["running"] == 2)
        .await;
    assert_eq!(pushed["team"]["queue"], json!([]));
    until_state(&p.d, &dev, &["ready"]).await;
    c.wait_for(|v| {
        v["type"] == "delivery_update"
            && v["delivery"]["bot_id"] == dev.as_str()
            && v["delivery"]["state"] == "delivered"
    })
    .await;
    // With the message read, it is idle again, and sleeps.
    until_state(&p.d, &dev, &["asleep"]).await;
    until_team(&mut c, |t| t["running"] == 1).await;
}

/// A routine falling due wakes its sleeping bot, and its prompt lands.
#[tokio::test]
async fn a_routine_falling_due_wakes_its_bot() {
    let (p, _bots) = project_with_bots_on(daemon(1).await, &["lead", "dev"]).await;
    let dev = p.ids[1].clone();
    let mut c = WsClient::connect(&p.d).await;
    until_state(&p.d, &dev, &["asleep"]).await;
    let routine = c
        .request(json!({
            "type": "create_routine", "bot_id": dev, "name": "daily",
            "trigger": {"kind": "interval", "seconds": 3600},
            "prompt": "/daily", "overlap_policy": "skip"
        }))
        .await;
    assert_eq!(routine["type"], "routine", "{routine}");
    let routine_id = routine["routine"]["id"].as_str().unwrap().to_string();

    c.request(json!({"type": "run_routine_now", "routine_id": routine_id}))
        .await;
    c.wait_for(|v| {
        v["type"] == "delivery_update"
            && v["delivery"]["bot_id"] == dev.as_str()
            && v["delivery"]["state"] == "delivered"
    })
    .await;
    assert_ne!(state(&p.d, &dev), "asleep");
}

/// The owner's controls: `wake_bot`, `sleep_bot` and `set_bot_presence`,
/// with the lead always on, and `team_updated` on each change.
#[tokio::test]
async fn the_owner_wakes_sleeps_and_keeps_bots_on() {
    // Nobody sleeps on their own here: only when the owner says so.
    let (p, _bots) = project_with_bots_on(daemon(600).await, &["lead", "dev"]).await;
    let (lead, dev) = (p.ids[0].clone(), p.ids[1].clone());
    let mut c = WsClient::connect(&p.d).await;
    until_state(&p.d, &dev, &["ready"]).await;

    let refused = c
        .request(json!({"type": "set_bot_presence", "bot_id": lead, "presence": "on_demand"}))
        .await;
    assert_eq!(refused["code"], "conflict", "{refused}");
    let refused = c
        .request(json!({"type": "sleep_bot", "bot_id": lead}))
        .await;
    assert_eq!(refused["code"], "conflict", "{refused}");
    let bad = c
        .request(json!({"type": "set_bot_presence", "bot_id": dev, "presence": "sometimes"}))
        .await;
    assert_eq!(bad["code"], "invalid_request", "{bad}");

    let ok = c.request(json!({"type": "sleep_bot", "bot_id": dev})).await;
    assert_eq!(ok["type"], "ok", "{ok}");
    until_state(&p.d, &dev, &["asleep"]).await;
    c.wait_for(|v| v["type"] == "team_updated" && v["team"]["running"] == 1)
        .await;

    let ok = c.request(json!({"type": "wake_bot", "bot_id": dev})).await;
    assert_eq!(ok["type"], "ok", "{ok}");
    c.wait_for(|v| v["type"] == "team_updated" && v["team"]["running"] == 2)
        .await;
    until_state(&p.d, &dev, &["ready"]).await;

    // Kept on: it refuses sleep, and stays up.
    let kept = c
        .request(json!({"type": "set_bot_presence", "bot_id": dev, "presence": "always_on"}))
        .await;
    assert_eq!(kept["type"], "bot", "{kept}");
    assert_eq!(kept["bot"]["presence"], "always_on");
    let refused = c.request(json!({"type": "sleep_bot", "bot_id": dev})).await;
    assert_eq!(refused["code"], "conflict", "{refused}");

    // On demand again, it can be sent to sleep.
    let back = c
        .request(json!({"type": "set_bot_presence", "bot_id": dev, "presence": "on_demand"}))
        .await;
    assert_eq!(back["bot"]["presence"], "on_demand", "{back}");
    c.request(json!({"type": "sleep_bot", "bot_id": dev})).await;
    until_state(&p.d, &dev, &["asleep"]).await;
    let team = until_team(&mut c, |t| t["running"] == 1).await;
    assert_eq!(team["queue"], json!([]));
}

/// `set_config` takes the cap and the idle time in range, echoes them, and
/// keeps them across a restart of the daemon's settings.
#[tokio::test]
async fn set_config_takes_the_cap_and_the_idle_time() {
    let d = daemon(600).await;
    let mut c = WsClient::connect(&d).await;
    let set = c
        .request(json!({"type": "set_config", "max_running_bots": 3, "idle_sleep_minutes": 20}))
        .await;
    assert_eq!(set["type"], "config", "{set}");
    assert_eq!(set["config"]["max_running_bots"], 3);
    assert_eq!(set["config"]["idle_sleep_minutes"], 20);
    let pushed = c.wait_for(|v| v["type"] == "team_updated").await;
    assert_eq!(pushed["team"]["max_running"], 3);
    for bad in [
        json!({"max_running_bots": 0}),
        json!({"max_running_bots": 33}),
        json!({"idle_sleep_minutes": 1441}),
        json!({"idle_sleep_minutes": "ten"}),
    ] {
        let mut req = bad.clone();
        req["type"] = json!("set_config");
        let refused = c.request(req).await;
        assert_eq!(refused["code"], "invalid_request", "{bad}: {refused}");
    }
    assert_eq!(
        d.app.db.get_meta("max_running_bots").unwrap().as_deref(),
        Some("3")
    );
    assert_eq!(
        d.app.db.get_meta("idle_sleep_minutes").unwrap().as_deref(),
        Some("20")
    );
    let config = c.request(json!({"type": "get_config"})).await;
    assert_eq!(config["config"]["max_running_bots"], 3);
}
