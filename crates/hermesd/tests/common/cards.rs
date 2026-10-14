//! A fresh project as the owner starts one (0.19.0 card flow): its bots
//! created from the app, its first board started by the owner's first
//! `board_get`, and the owner's own items, assignments and moves over
//! WebSocket.

use bus::contract::board::{self as c, board_request::Request, board_response::Response};
use bus::contract::wire::envelope::Body;
use serde_json::{json, Value};

use super::board::{call, move_to, moved, response, snapshot};
use super::k9::inbox_until;
use super::{create_bot, peers, spawn_daemon, McpClient, TestDaemon, WsClient};

pub struct Fresh {
    pub d: TestDaemon,
    pub owner: WsClient,
    pub project: String,
    pub bot_ids: Vec<String>,
    pub bots: Vec<McpClient>,
}

/// A project named "Shop" with the named bots, created in order from the
/// app, and its board as the owner first opens it.
pub async fn fresh(names: &[&str]) -> (Fresh, c::BoardSnapshot) {
    let d = spawn_daemon().await;
    let mut owner = WsClient::connect(&d).await;
    let project = peers::project(&mut owner, "Shop").await;
    let mut bot_ids = Vec::new();
    let mut bots = Vec::new();
    for name in names {
        let bot = create_bot(&mut owner, &project, name).await;
        let id = bot["id"].as_str().expect("id").to_string();
        let token = d.app.secrets.bot_token(&id).expect("token");
        bots.push(McpClient::new(&d, &token));
        bot_ids.push(id);
    }
    let mut f = Fresh {
        d,
        owner,
        project,
        bot_ids,
        bots,
    };
    let s = f.board().await;
    (f, s)
}

impl Fresh {
    pub async fn board(&mut self) -> c::BoardSnapshot {
        let project_id = self.project.clone();
        snapshot(
            call(
                &mut self.owner,
                Request::BoardGet(c::BoardGet { project_id }),
            )
            .await,
        )
    }

    /// The owner's new live item, in the board's first open column.
    pub async fn item(&mut self, title: &str) -> c::Item {
        let req = Request::ItemCreate(c::ItemCreate {
            project_id: self.project.clone(),
            r#type: c::ItemType::Feature as i32,
            title: title.into(),
            description: Some("Two button styles for the shop page.".into()),
            ..Default::default()
        });
        done(call(&mut self.owner, req).await)
    }

    /// The item as it is now.
    pub fn now(&self, id: &str) -> hermesd::kanban::model::Item {
        self.d.app.db.get_item(id).unwrap().expect("item")
    }

    /// The owner assigns the item to a bot, or takes it off when `None`.
    pub async fn assign(&mut self, id: &str, bot: Option<&str>) -> c::Item {
        let req = Request::ItemAssign(c::ItemAssign {
            id: id.into(),
            expected_version: self.now(id).version,
            bot: bot.map(str::to_string),
        });
        done(call(&mut self.owner, req).await)
    }

    /// The owner moves the item, with a reason when given.
    pub async fn owner_move(&mut self, id: &str, to: &str, reason: Option<&str>) -> c::MoveResult {
        let version = self.now(id).version;
        c::MoveResult {
            outcome: Some(moved(
                call(&mut self.owner, move_to(id, to, version, reason)).await,
            )),
        }
    }

    /// The bot `i` moves the item over MCP; the raw result.
    pub async fn bot_move(&mut self, i: usize, id: &str, to: &str) -> Value {
        let version = self.now(id).version;
        self.bots[i]
            .call_raw(
                "item_move",
                json!({"id": id, "to": to, "expected_version": version}),
            )
            .await
    }

    /// Reads bot `i`'s inbox until a message holds `needle`; that message.
    pub async fn heard(&mut self, i: usize, needle: &str) -> String {
        inbox_until(&mut self.bots[i], needle)
            .await
            .into_iter()
            .find(|b| b.contains(needle))
            .expect("the message")
    }

    /// The card of `id` in the owner's board as it is now.
    pub async fn card(&mut self, id: &str) -> c::ItemCard {
        let s = self.board().await;
        s.cards
            .into_iter()
            .find(|card| card.id == id)
            .unwrap_or_else(|| panic!("no card {id}"))
    }
}

/// The item a WebSocket edit answered with.
pub fn done(body: Body) -> c::Item {
    match response(body) {
        Response::Edited(c::EditResult {
            outcome: Some(c::edit_result::Outcome::Done(item)),
            ..
        }) => item,
        other => panic!("expected the item, got {other:?}"),
    }
}

/// The columns of the default board, by name, visible or not.
pub fn column_names(s: &c::BoardSnapshot) -> Vec<String> {
    let main = s
        .boards
        .iter()
        .find(|b| b.is_default)
        .expect("a default board");
    s.columns
        .iter()
        .filter(|col| col.board_id == main.id)
        .map(|col| col.name.clone())
        .collect()
}
