//! The K9 acceptance scenario's fixture (0.19.0): a fresh project, "Recipe
//! Box", homed on the Mac and linked from the PC, with the lead on the Mac
//! and the cook on the PC. The owner works from the desktop app (a ticket)
//! and from the phone (a paired device); the bots are MCP clients.

pub mod drafts;
pub mod flow;
pub mod scan;

use bus::contract::board::{
    self as c, board_request::Request, board_response::Response, structure_result::Outcome as W,
};
use bus::contract::wire::envelope::Body;
use serde_json::{json, Value};

use super::board::{call, connect_with, response, snapshot};
use super::peers::{bot_named, paired, project, wait_until, Paired};
use super::{create_bot, token_str, McpClient, WsClient};

pub struct K9 {
    /// The Mac (the home) and the PC; `p.mac_client` is the desktop app.
    pub p: Paired,
    /// The owner's phone: a device paired with the Mac.
    pub phone: WsClient,
    pub mac_rb: String,
    pub win_rb: String,
    pub lead: McpClient,
    pub lead_id: String,
    /// The cook, on the PC, and its stand-in on the Mac.
    pub cook: McpClient,
    pub cook_id: String,
    pub cook_on_mac: String,
    /// Every message either bot read, for step 8.
    pub told: Vec<String>,
}

/// Step 1: the project, its two bots and the lead, and the board started on
/// the Mac, as the desktop's "Start the board on this computer" does.
pub async fn setup() -> K9 {
    let mut p = paired().await;
    let mac_rb = project(&mut p.mac_client, "Recipe Box").await;
    let lead = create_bot(&mut p.mac_client, &mac_rb, "Lead").await;
    let lead_id = lead["id"].as_str().expect("id").to_string();
    let set = p
        .mac_client
        .request(json!({"type": "set_project_lead", "project_id": mac_rb, "bot_id": lead_id}))
        .await;
    assert_ne!(set["type"], "error", "{set}");
    let win_rb = project(&mut p.win_client, "Recipe Box").await;
    let cook = create_bot(&mut p.win_client, &win_rb, "Cook").await;
    let cook_id = cook["id"].as_str().expect("id").to_string();
    let linked = p
        .mac_client
        .request(json!({"type": "link_project", "project_id": mac_rb,
                        "peer_id": p.mac_peer_id, "remote_project_id": win_rb}))
        .await;
    assert_eq!(linked["type"], "project", "{linked}");
    let mac = &p.mac;
    wait_until("the cook stands in on the Mac", || {
        bot_named(mac, &mac_rb, "Cook").is_some()
    })
    .await;
    let cook_on_mac = bot_named(mac, &mac_rb, "Cook").expect("stand-in").id;

    let started = call(
        &mut p.mac_client,
        Request::BoardEnable(c::BoardEnable {
            project_id: mac_rb.clone(),
            ..Default::default()
        }),
    )
    .await;
    let started = snapshot(started);
    let home = started.settings.expect("settings").home_daemon_id;
    assert_eq!(home, mac.app.db.daemon_id().unwrap());

    let device = p
        .mac_client
        .request(json!({"type": "create_device", "name": "phone",
                        "capabilities": ["read", "control", "approve"]}))
        .await;
    assert_eq!(device["type"], "device", "{device}");
    let phone = connect_with(mac, token_str(&device)).await;
    let lead_token = mac.app.secrets.bot_token(&lead_id).expect("token");
    let cook_token = p.win.app.secrets.bot_token(&cook_id).expect("token");
    K9 {
        lead: McpClient::new(mac, &lead_token),
        cook: McpClient::new(&p.win, &cook_token),
        p,
        phone,
        mac_rb,
        win_rb,
        lead_id,
        cook_id,
        cook_on_mac,
        told: Vec::new(),
    }
}

impl K9 {
    /// The project's boards as the desktop app reads them.
    pub async fn desktop_board(&mut self) -> c::BoardSnapshot {
        let get = Request::BoardGet(c::BoardGet {
            project_id: self.mac_rb.clone(),
        });
        snapshot(call(&mut self.p.mac_client, get).await)
    }

    /// Reads the lead's inbox until a message contains `needle`; returns it.
    pub async fn lead_heard(&mut self, needle: &str) -> String {
        let read = inbox_until(&mut self.lead, needle).await;
        self.told.extend(read.iter().cloned());
        read.into_iter()
            .find(|b| b.contains(needle))
            .expect("the message")
    }

    /// Waits until the PC's mirror of the board satisfies `check`.
    pub async fn pc_mirrors(&self, what: &str, check: impl Fn(&c::BoardSnapshot) -> bool) {
        let (win, win_rb) = (&self.p.win.app, self.win_rb.clone());
        wait_until(what, || {
            win.board_mirror
                .get(&win_rb)
                .is_some_and(|m| check(&m.snapshot))
        })
        .await;
    }
}

/// The board named `name` in a snapshot.
pub fn board<'a>(s: &'a c::BoardSnapshot, name: &str) -> &'a c::Board {
    s.boards
        .iter()
        .find(|b| b.name == name)
        .unwrap_or_else(|| panic!("no board {name} in {:?}", s.boards))
}

/// The key of the column named `name` on `board_id`.
pub fn column(s: &c::BoardSnapshot, board_id: &str, name: &str) -> String {
    s.columns
        .iter()
        .find(|col| col.board_id == board_id && col.name == name)
        .unwrap_or_else(|| panic!("no column {name} on {board_id}"))
        .key
        .clone()
}

/// A structure change the owner made, applied.
pub fn applied(body: Body) -> c::BoardSnapshot {
    match response(body) {
        Response::Structure(c::StructureResult {
            outcome: Some(W::Applied(s)),
            ..
        }) => s,
        other => panic!("expected an applied change, got {other:?}"),
    }
}

/// A lead's structure tool call that became a proposal: its id.
pub fn proposed(reply: &Value) -> String {
    assert_eq!(reply["applied"], false, "expected a proposal: {reply}");
    reply["proposal_id"]
        .as_str()
        .expect("proposal id")
        .to_string()
}

/// Approves a proposal from an owner's app, as Needs you does.
pub async fn approve(app: &mut WsClient, proposal_id: &str) -> c::BoardSnapshot {
    applied(
        call(
            app,
            Request::BoardProposalRule(c::BoardProposalRule {
                proposal_id: proposal_id.to_string(),
                approve: true,
                note: None,
            }),
        )
        .await,
    )
}

/// The refusal text of a tool call that was refused.
pub fn refusal(raw: &Value) -> String {
    assert_eq!(raw["isError"], json!(true), "expected a refusal: {raw}");
    raw["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

/// Reads a bot's inbox, the daemon's own notes included, until a message
/// contains `needle`; returns every body read.
pub async fn inbox_until(bot: &mut McpClient, needle: &str) -> Vec<String> {
    let mut seen = Vec::new();
    for _ in 0..60 {
        let inbox = bot.call("check_inbox", json!({})).await;
        let bodies = inbox["messages"].as_array().into_iter().flatten();
        seen.extend(bodies.filter_map(|m| m["body"].as_str().map(str::to_string)));
        if seen.iter().any(|b| b.contains(needle)) {
            return seen;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    panic!("no message containing {needle:?} in {seen:?}");
}

/// An MCP item's version: proto3 JSON writes 64-bit integers as strings.
pub fn version_of(item: &Value) -> u64 {
    super::board::version(item)
}
