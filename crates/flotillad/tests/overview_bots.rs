//! The Board glance's projects home (0.20.0): every bot of a project across
//! its computers (`overview_bots`), the project's first row
//! (`overview_top_attention`), stopped bots (`attention_bot_stopped`) and
//! who is online, and the peer `revision` that keeps a 0.19 caller reading.

use crate::common;

use bus::contract::home::{
    AttentionKind, BotBrief, BotStatus, ProjectAttention, ProjectRow, ProjectsOverview,
};
use bus::BotState;
use common::peer_board::board;
use common::peers::{project, wait_until};
use common::*;
use flotillad::app::AppState;
use serde_json::json;

fn overview(app: &AppState) -> ProjectsOverview {
    flotillad::overview::overview(app, &[]).expect("overview")
}

fn row(app: &AppState, project_id: &str) -> ProjectRow {
    overview(app)
        .rows
        .into_iter()
        .find(|r| r.project_id == project_id)
        .unwrap_or_else(|| panic!("no row for {project_id}"))
}

fn named<'a>(row: &'a ProjectRow, name: &str) -> Option<&'a BotBrief> {
    row.bot_list.iter().find(|b| b.name == name)
}

/// Down for two minutes, as the supervisor would have it after a crash.
fn crash(app: &AppState, bot_id: &str) {
    app.supervisor
        .set_state(bot_id, BotState::Crashed, "Engine exited with code 1");
    let since = chrono::Utc::now() - chrono::Duration::minutes(2);
    app.overview
        .set_down_for_tests(bot_id, BotState::Crashed, since);
}

#[tokio::test]
async fn a_crashed_bot_tops_its_project_until_it_runs_again() {
    let d = spawn_daemon().await;
    let mut owner = WsClient::connect(&d).await;
    let pid = project(&mut owner, "Glance").await;
    let lead = create_bot(&mut owner, &pid, "Lead").await;
    let worker = create_bot(&mut owner, &pid, "worker").await;
    let (lead, worker) = (
        lead["id"].as_str().unwrap().to_string(),
        worker["id"].as_str().unwrap().to_string(),
    );
    let me = d.app.db.daemon_id().unwrap();

    let calm = row(&d.app, &pid);
    assert_eq!(calm.bot_list.len(), 2);
    assert_eq!(calm.bot_list[0].bot_id, lead, "the lead first");
    assert!(calm.bot_list[0].lead);
    assert_eq!(calm.bot_list[1].daemon_id, me);
    assert_eq!(calm.bot_list[1].runtime, "claude_code");
    assert_eq!(calm.members.len(), 1);
    assert!(calm.members[0].online);

    crash(&d.app, &worker);
    let down = row(&d.app, &pid);
    let brief = named(&down, "worker").expect("worker");
    assert_eq!(brief.status(), BotStatus::Failed);
    assert_eq!(brief.state, "crashed");
    let top = down.top_attention.expect("a first row");
    assert_eq!(top.kind(), AttentionKind::BotStopped);
    assert_eq!(top.bot_state, "crashed");
    assert_eq!(top.id, format!("bot_stopped:{me}:{worker}"));
    assert_eq!(top.weight, 2);
    assert!(top.title.starts_with("worker crashed"), "{}", top.title);
    let rows = flotillad::overview::attention_rows(&d.app, &pid).unwrap();
    assert_eq!(rows.rows[0].id, top.id);
    assert_eq!(
        down.attention.unwrap().by_kind.get("bot_stopped").copied(),
        Some(1)
    );

    // A bot that only just stopped has no row yet.
    d.app
        .supervisor
        .set_state(&lead, BotState::Stopped, "session ended");
    let fresh = row(&d.app, &pid);
    assert_eq!(named(&fresh, "Lead").unwrap().status(), BotStatus::Stopped);
    let stopped = fresh.attention.unwrap().by_kind.get("bot_stopped").copied();
    assert_eq!(stopped, Some(1), "only the worker's");

    // Running again: the row is gone.
    d.app.supervisor.set_state(&worker, BotState::Ready, "");
    let back = row(&d.app, &pid);
    assert_eq!(named(&back, "worker").unwrap().status(), BotStatus::Idle);
    assert!(back
        .top_attention
        .is_none_or(|r| r.kind() != AttentionKind::BotStopped));
}

#[tokio::test]
async fn a_project_without_bots_or_rows_has_neither() {
    let d = spawn_daemon().await;
    let mut owner = WsClient::connect(&d).await;
    let pid = project(&mut owner, "Empty").await;
    let empty = row(&d.app, &pid);
    assert!(empty.bot_list.is_empty());
    assert!(empty.top_attention.is_none());
}

/// The Mac lists the PC's bots from the PC's own part, once, with the
/// computer's presence; a 0.19 caller (no revision) gets neither the bots
/// nor a stopped row.
#[tokio::test]
async fn linked_bots_are_listed_once_from_their_own_computer() {
    let b = board().await;
    let (mac, win) = (&b.p.mac.app, &b.p.win.app);
    let win_daemon = win.db.daemon_id().unwrap();
    let mac_app = b.mac_app.clone();
    wait_until("the Mac lists the PC's tester", || {
        named(&row(mac, &mac_app), "tester")
            .is_some_and(|t| t.daemon_id == win_daemon && t.status() != BotStatus::Unspecified)
    })
    .await;
    let mac_row = row(mac, &b.mac_app);
    let tester = named(&mac_row, "tester").unwrap();
    assert_eq!(tester.bot_id, b.tester_id, "the PC's own id");
    assert_eq!(tester.computer_name, "win");
    assert_eq!(
        mac_row
            .bot_list
            .iter()
            .filter(|bot| bot.name == "tester")
            .count(),
        1,
        "not its stand-in too"
    );
    assert_eq!(mac_row.bot_list[0].name, "lead");
    let win_member = mac_row
        .members
        .iter()
        .find(|m| m.daemon_id == win_daemon)
        .expect("the PC");
    assert!(win_member.online);
    assert!(win_member.last_seen_at.is_some());

    crash(win, &b.tester_id);
    let ask = |revision: Option<u32>| {
        let mut frame = json!({"type": "project_attention", "project_ids": [b.win_app]});
        if let Some(revision) = revision {
            frame["revision"] = json!(revision);
        }
        let (mac, peer) = (mac.clone(), b.p.mac_peer_id.clone());
        async move {
            let answer = mac.peers.request(&peer, frame).await.expect("an answer");
            serde_json::from_value::<ProjectAttention>(answer).expect("proto3 JSON")
        }
    };
    let new = ask(Some(1)).await;
    assert!(!new.parts[0].bots.is_empty());
    assert!(new.parts[0]
        .rows
        .iter()
        .any(|r| r.kind() == AttentionKind::BotStopped));
    let old = ask(None).await;
    assert!(old.parts[0].bots.is_empty());
    assert!(old.parts[0]
        .rows
        .iter()
        .all(|r| r.kind() != AttentionKind::BotStopped));
    let local = old.parts[0].local.clone().unwrap();
    assert!(!local.by_kind.contains_key("bot_stopped"));

    // The Mac hears of it and tops the row with the PC's.
    wait_until("the Mac tops its row with the PC's stopped bot", || {
        row(mac, &mac_app)
            .top_attention
            .is_some_and(|r| r.kind() == AttentionKind::BotStopped && r.daemon_id == win_daemon)
    })
    .await;

    // The PC goes away: its bots read offline.
    let win_peer_id = b.p.win_peer_id.clone();
    win.db.revoke_peer(&win_peer_id).unwrap();
    win.peers.disconnect(&win_peer_id);
    wait_until("the Mac shows the PC's bots offline", || {
        named(&row(mac, &mac_app), "tester").is_some_and(|t| t.status() == BotStatus::Offline)
    })
    .await;
    let away = row(mac, &b.mac_app);
    assert!(
        !away
            .members
            .iter()
            .find(|m| m.daemon_id == win_daemon)
            .unwrap()
            .online
    );
}
