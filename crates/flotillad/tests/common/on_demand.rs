//! A daemon whose on-demand bots sleep after a second idle, ticking fast,
//! for the team on demand tests (0.23.0).

use std::time::Duration;

use flotillad::actor::Actor;
use flotillad::db::NewItem;
use flotillad::kanban::model::{ItemType, Priority};
use serde_json::{json, Value};

use super::tasks::{project_with_bots_on, Pair};
use super::{frame_wait, spawn_daemon_with, McpClient, TestDaemon, WsClient};

/// A daemon ticking every 100 ms whose on-demand bots sleep after
/// `idle_seconds` idle.
pub async fn daemon(idle_seconds: u64) -> TestDaemon {
    spawn_daemon_with(|cfg| {
        cfg.supervision_interval_ms = 100;
        cfg.team.idle_sleep_seconds_for_tests = Some(idle_seconds);
    })
    .await
}

/// A project of these bots on such a daemon, its first one the lead, with a
/// board whose cards are `H-n`.
pub async fn team(idle_seconds: u64, names: &[&str]) -> (Pair, Vec<McpClient>, String) {
    let (pair, bots) = project_with_bots_on(daemon(idle_seconds).await, names).await;
    let db = &pair.d.app.db;
    let project = db.get_bot(&pair.ids[0]).unwrap().unwrap().project_id;
    db.ensure_board(&project, &db.daemon_id().unwrap(), Some("H"))
        .unwrap();
    (pair, bots, project)
}

/// A card on the board's first column.
pub fn card(d: &TestDaemon, project: &str, item_type: ItemType, priority: Priority) -> String {
    d.app
        .db
        .create_item(
            &NewItem {
                project_id: project,
                board_id: None,
                item_type,
                title: "On demand",
                description: "",
                size: None,
                priority,
                labels: &["platform:daemon".to_string()],
                parent_id: None,
                acceptance_criteria: &[],
                plan_node_id: None,
            },
            &Actor::User,
        )
        .unwrap()
        .id
}

/// The bot's state, as the supervisor has it.
pub fn state(d: &TestDaemon, bot_id: &str) -> String {
    d.app.supervisor.state(bot_id).0.as_str().to_string()
}

/// Waits until the bot is in one of `states`.
pub async fn until_state(d: &TestDaemon, bot_id: &str, states: &[&str]) {
    let deadline = tokio::time::Instant::now() + frame_wait();
    loop {
        let now = state(d, bot_id);
        if states.contains(&now.as_str()) {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "{bot_id} stayed {now}, never {states:?}"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// `team_status`, asked until `pred` holds of its `team`.
pub async fn until_team(c: &mut WsClient, pred: impl Fn(&Value) -> bool) -> Value {
    let deadline = tokio::time::Instant::now() + frame_wait();
    loop {
        let reply = c.request(json!({"type": "team_status"})).await;
        assert_eq!(reply["type"], "team", "{reply}");
        if pred(&reply["team"]) {
            return reply["team"].clone();
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "the team never got there: {}",
            reply["team"]
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// The queue's bots, in the order they will start.
pub fn queue(team: &Value) -> Vec<String> {
    team["queue"]
        .as_array()
        .expect("queue")
        .iter()
        .map(|w| w["bot_id"].as_str().expect("bot_id").to_string())
        .collect()
}

/// `from` sends the bot named `to` a message of `kind` (`note`, `task`).
pub async fn dm(from: &mut McpClient, to: &str, kind: &str, body: &str) {
    from.call(
        "send_message",
        json!({"to": to, "kind": kind, "body": body}),
    )
    .await;
}

/// `from` gives the bot named `to` a task on `item`.
pub async fn task(from: &mut McpClient, to: &str, item: &str, body: &str) {
    from.call(
        "send_message",
        json!({"to": to, "kind": "task", "item": item, "body": body}),
    )
    .await;
}
