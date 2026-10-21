//! `commands_changed` (0.24.0, `commands_v1`): clients refetch
//! `list_commands` when a bot's commands change, where they used to poll.
//! A command starting, ending or being stopped is a change; one printing is
//! too, at most every few seconds. Every chat a client has read stays
//! current (`chat::watch`), and this reads what it found. A bot linked to
//! another computer is announced there too, which relays it as its own.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use bus::Bot;
use serde_json::{json, Value};

use crate::app::AppState;
use crate::events::Push;

/// How often the loaded chats' command logs are looked at.
const TICK: Duration = Duration::from_secs(1);
/// A command printing is news at most this often per bot.
const PRINTS_EVERY: Duration = Duration::from_secs(5);

/// The peer event that carries it.
pub const EVENT: &str = "commands_changed";

/// Tells this computer's clients, and the computers the bot is linked to,
/// that the bot's commands changed.
pub fn changed(app: &AppState, bot: &Bot) {
    let Ok(daemon_id) = app.db.daemon_id() else {
        return;
    };
    app.events.push(Push::CommandsChanged {
        bot_id: bot.id.clone(),
        project_id: bot.project_id.clone(),
        daemon_id,
    });
    for peer in app.db.peers_exposed_to(&bot.id).unwrap_or_default() {
        app.peers
            .notify(&peer, json!({ "type": EVENT, "bot_id": bot.id }));
    }
}

/// A linked computer's bot's commands changed: its stand-ins' here did.
pub fn receive(app: &AppState, peer_id: &str, frame: &Value) {
    let remote = frame["bot_id"].as_str().unwrap_or_default();
    let Ok(Some(peer)) = app.db.get_peer(peer_id) else {
        return;
    };
    let daemon_id = peer.daemon_id.unwrap_or_else(|| peer_id.to_string());
    for bot in app.db.linked_bots_for(peer_id, remote).unwrap_or_default() {
        app.events.push(Push::CommandsChanged {
            bot_id: bot.id,
            project_id: bot.project_id,
            daemon_id: daemon_id.clone(),
        });
    }
}

/// What was last seen of one bot's commands.
struct Seen {
    changes: u64,
    printed: u64,
    told_print: Instant,
}

/// The size of everything the bot's running commands printed.
fn printed(outputs: &[String]) -> u64 {
    outputs
        .iter()
        .filter_map(|f| std::fs::metadata(f).ok())
        .map(|m| m.len())
        .sum()
}

pub async fn run(app: Arc<AppState>) {
    let mut seen: HashMap<String, Seen> = HashMap::new();
    let mut tick = tokio::time::interval(TICK);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tick.tick().await;
        let app = app.clone();
        let mut was = std::mem::take(&mut seen);
        seen = tokio::task::spawn_blocking(move || {
            look(&app, &mut was);
            was
        })
        .await
        .unwrap_or_default();
    }
}

fn look(app: &AppState, seen: &mut HashMap<String, Seen>) {
    let loaded = app.chat.loaded();
    seen.retain(|bot, _| loaded.contains(bot));
    for bot_id in loaded {
        let Some((changes, base, outputs)) = app.chat.command_state(&bot_id) else {
            continue;
        };
        let printed = printed(&outputs);
        // What the chat's first read found is a snapshot, not news; what
        // came after it is, even before this first look.
        let was = seen.entry(bot_id.clone()).or_insert_with(|| Seen {
            changes: base,
            printed,
            told_print: Instant::now(),
        });
        let moved = changes != was.changes;
        let prints = printed != was.printed && was.told_print.elapsed() >= PRINTS_EVERY;
        if !moved && !prints {
            continue;
        }
        *was = Seen {
            changes,
            printed,
            told_print: if prints {
                Instant::now()
            } else {
                was.told_print
            },
        };
        if let Ok(Some(bot)) = app.db.get_live_bot(&bot_id) {
            changed(app, &bot);
        }
    }
}
