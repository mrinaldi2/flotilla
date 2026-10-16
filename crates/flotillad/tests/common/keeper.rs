//! The board keeper on a test's clock (0.19.0). Time passes by moving a
//! card's stored times (its history, comments, entry into its column and
//! the keeper's marks on it) into the past, then stepping the keeper at the
//! real now with a memory of the test's: what a daemon that ran that long
//! would see.

use std::sync::Arc;

use chrono::{Duration, Utc};
use flotillad::app::AppState;
use flotillad::kanban::keeper::{self, Keeper};
use serde_json::json;

use super::cards::Fresh;
use super::{create_bot, peers, spawn_daemon, McpClient};

/// A keeper (a daemon's memory) that started long ago, so no timer waits
/// on its start.
pub fn long_running() -> Keeper {
    Keeper::booted_at(Utc::now() - Duration::days(365))
}

/// A keeper (a daemon) that started `ago`.
pub fn started(ago: Duration) -> Keeper {
    Keeper::booted_at(Utc::now() - ago)
}

/// The daemon's own keeper has been running for a year: it is the one that
/// saw the bots' reads and turns.
pub fn up_long(app: &AppState) -> &Keeper {
    app.keeper
        .running_since_for_tests(Utc::now() - Duration::days(365));
    &app.keeper
}

/// One pass of `k` at the real now.
pub fn step(app: &Arc<AppState>, k: &Keeper) {
    keeper::step(app, k, Utc::now()).expect("a keeper pass");
}

/// `by` passes for the card: every time stored about it moves back.
pub fn age(app: &AppState, item_id: &str, by: Duration) {
    let raw = rusqlite::Connection::open(app.cfg.db_path()).expect("db");
    let shift = format!("-{} seconds", by.num_seconds());
    let moved = |col: &str| format!("{col} = strftime('%Y-%m-%dT%H:%M:%f+00:00', {col}, ?2)");
    for (table, cols, key) in [
        (
            "item",
            vec!["state_entered_at", "created_at", "updated_at"],
            "id",
        ),
        ("item_event", vec!["at"], "item_id"),
        ("item_comment", vec!["at"], "item_id"),
        ("keeper_mark", vec!["since", "fired_at"], "item_id"),
    ] {
        let set: Vec<String> = cols.iter().map(|c| moved(c)).collect();
        raw.execute(
            &format!("UPDATE {table} SET {} WHERE {key} = ?1", set.join(", ")),
            rusqlite::params![item_id, shift],
        )
        .expect("aged");
    }
}

/// The keeper's notes to the bot, oldest first.
pub fn notes(app: &AppState, bot_id: &str) -> Vec<String> {
    let conv = app.db.dm_conversation(bot_id).unwrap().expect("dm");
    app.db
        .list_messages(&conv.id, None, 500)
        .unwrap()
        .into_iter()
        .filter(|m| m.sender.name == "system" && m.body.contains("] Automatic "))
        .map(|m| m.body)
        .collect()
}

/// The project's Needs-you rows the keeper raised: (id, title).
pub fn rows(app: &AppState, project_id: &str) -> Vec<(String, String)> {
    flotillad::overview::attention_rows(app, project_id)
        .unwrap()
        .rows
        .into_iter()
        .filter(|r| r.id.starts_with("card_waiting:"))
        .map(|r| (r.id, r.title))
        .collect()
}

/// The keeper's marks on the card, by rule.
pub fn marks(app: &AppState, project_id: &str, item_id: &str) -> Vec<String> {
    app.db
        .keeper_marks(project_id)
        .unwrap()
        .into_iter()
        .filter(|m| m.item_id == item_id)
        .map(|m| m.rule)
        .collect()
}

impl Fresh {
    /// The owner's card, assigned to bot `i` and started by its read: in
    /// Doing.
    pub async fn started_card(&mut self, i: usize, title: &str) -> String {
        let item = self.item(title).await;
        let name = self
            .d
            .app
            .db
            .get_bot(&self.bot_ids[i])
            .unwrap()
            .unwrap()
            .name;
        self.assign(&item.id, Some(&name)).await;
        let got = self.bots[i].call("item_get", json!({"id": item.id})).await;
        assert_eq!(got["item"]["column_key"], "doing", "{got}");
        item.id
    }

    /// Bot `i` starts a turn, once it is up.
    pub async fn turn_starts(&self, i: usize) {
        let bot_id = self.bot_ids[i].clone();
        let sup = &self.d.app.supervisor;
        peers::wait_until("the bot is up", || {
            sup.state(&bot_id).0 == bus::BotState::Ready
        })
        .await;
        sup.on_hook(&bot_id, "UserPromptSubmit", None);
    }

    /// Bot `i`'s turn ends; returns once the keeper has judged it.
    pub async fn turn_ends(&self, i: usize) {
        let keeper = &self.d.app.keeper;
        let before = keeper.turns_judged();
        self.d
            .app
            .supervisor
            .on_hook(&self.bot_ids[i], "Stop", None);
        peers::wait_until("the keeper judged the turn", || {
            keeper.turns_judged() > before
        })
        .await;
    }
}

/// [`super::cards::fresh`] on a release-gated Main, as projects made before
/// 0.19 have.
pub async fn gated(names: &[&str]) -> Fresh {
    let d = spawn_daemon().await;
    let mut owner = super::WsClient::connect(&d).await;
    let project = peers::project(&mut owner, "Shop").await;
    let db = &d.app.db;
    db.ensure_board(&project, &db.daemon_id().unwrap(), Some("H"))
        .unwrap();
    let mut bot_ids = Vec::new();
    let mut bots = Vec::new();
    for name in names {
        let bot = create_bot(&mut owner, &project, name).await;
        let id = bot["id"].as_str().expect("id").to_string();
        let token = d.app.secrets.bot_token(&id).expect("token");
        bots.push(McpClient::new(&d, &token));
        bot_ids.push(id);
    }
    Fresh {
        d,
        owner,
        project,
        bot_ids,
        bots,
    }
}
