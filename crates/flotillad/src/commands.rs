//! Commands across every bot (0.24.0, `commands_v1`): the Commands page
//! that replaces Main chat. `list_commands` gathers each bot's commands
//! (`chat::commands`, read from its transcript) across projects, and a
//! linked computer's bots from that computer; `stop_command` is the owner's
//! Stop (`stop`); `commands_changed` tells clients a bot's list changed
//! (`watch`).
//!
//! A row's `bot_id` and `project_id` are this computer's (a linked bot's
//! stand-in here), so `stop_command` can name it; its `id` is the command's
//! own (the tool call's), and `daemon_id` the computer it runs on.

use std::cmp::Ordering;
use std::sync::Arc;
use std::time::Duration;

use bus::Bot;
use chrono::{DateTime, Local};
use serde_json::{json, Value};

use crate::app::AppState;
use crate::db::Db;

mod kill;
pub mod stop;
pub mod watch;

/// Rows on one page when the client names no limit, and at most.
pub const DEFAULT_LIMIT: usize = 200;
const MAX_LIMIT: usize = 500;
/// Commands read per bot: the newest, the running ones first.
const PER_BOT: usize = 500;
/// How long a linked computer has to answer before it shows as stale.
const PEER_WAIT: Duration = Duration::from_secs(5);

/// What `list_commands` asks for.
#[derive(Debug, Default, Clone)]
pub struct Filter {
    pub project_id: Option<String>,
    pub bot_id: Option<String>,
    /// `running`, `failed` or `all` (the default).
    pub status: Option<String>,
    /// Matches the command and its description, case-insensitively.
    pub query: Option<String>,
    pub limit: Option<usize>,
    /// The `next_before` of the page before.
    pub before: Option<String>,
}

/// One computer's bots' rows, or that it didn't answer.
struct Part {
    daemon_id: String,
    computer_name: String,
    rows: Vec<Value>,
    stale: bool,
}

/// The bots `filter` names: live, in a live project.
fn bots(app: &AppState, filter: &Filter) -> anyhow::Result<Vec<Bot>> {
    let bots = app.db.list_bots(filter.project_id.as_deref())?;
    let mut out = Vec::new();
    for bot in bots {
        if bot.deleted_at.is_some() || filter.bot_id.as_ref().is_some_and(|b| *b != bot.id) {
            continue;
        }
        if app.db.get_live_project(&bot.project_id)?.is_some() {
            out.push(bot);
        }
    }
    Ok(out)
}

/// A command as a row: the command's own fields, and its bot's.
fn row(mut command: Value, bot: &Bot, daemon_id: &str) -> Value {
    command["bot_id"] = json!(bot.id);
    command["bot_name"] = json!(bot.name);
    command["project_id"] = json!(bot.project_id);
    command["daemon_id"] = json!(daemon_id);
    command
}

/// This computer's bots' rows.
fn local(app: &AppState, bots: &[Bot]) -> anyhow::Result<Part> {
    let daemon_id = app.db.daemon_id()?;
    let mut rows = Vec::new();
    for bot in bots.iter().filter(|b| !b.is_linked()) {
        match app.chat.commands(app, bot, PER_BOT) {
            Ok(commands) => rows.extend(
                commands
                    .into_iter()
                    .filter_map(|c| serde_json::to_value(c).ok())
                    .map(|c| row(c, bot, &daemon_id)),
            ),
            Err(e) => tracing::debug!(bot_id = %bot.id, error = %e, "commands not read"),
        }
    }
    let computer_name = app
        .db
        .board_read(crate::releases::machines::this_computer)
        .unwrap_or_default();
    Ok(Part {
        daemon_id,
        computer_name,
        rows,
        stale: false,
    })
}

/// A linked computer's bots' rows, read from it; stale when it doesn't
/// answer for one of them.
async fn linked(app: &Arc<AppState>, peer_id: &str, bots: Vec<Bot>) -> Part {
    let peer = app.db.get_peer(peer_id).ok().flatten();
    let daemon_id = peer
        .as_ref()
        .and_then(|p| p.daemon_id.clone())
        .unwrap_or_else(|| peer_id.to_string());
    let computer_name = peer.as_ref().map(Db::display_peer_name).unwrap_or_default();
    let asks = bots.iter().map(|bot| {
        let frame = json!({ "type": "bot_commands", "limit": PER_BOT });
        tokio::time::timeout(PEER_WAIT, crate::peer::chat::ask(app, bot, frame))
    });
    let answers = futures::future::join_all(asks).await;
    let mut part = Part {
        daemon_id,
        computer_name,
        rows: Vec::new(),
        stale: false,
    };
    for (bot, answer) in bots.iter().zip(answers) {
        match answer {
            Ok(Ok(result)) => {
                let commands = result["commands"].as_array().cloned().unwrap_or_default();
                part.rows
                    .extend(commands.into_iter().map(|c| row(c, bot, &part.daemon_id)));
            }
            _ => part.stale = true,
        }
    }
    part
}

fn text<'a>(row: &'a Value, key: &str) -> &'a str {
    row[key].as_str().unwrap_or_default()
}

fn running(row: &Value) -> bool {
    text(row, "status") == "running"
}

/// Whether a row ended or started today, in this computer's time.
fn failed_today(row: &Value) -> bool {
    if text(row, "status") != "failed" {
        return false;
    }
    let at = row["ended_at"].as_str().unwrap_or(text(row, "started_at"));
    DateTime::parse_from_rfc3339(at)
        .is_ok_and(|at| at.with_timezone(&Local).date_naive() == Local::now().date_naive())
}

/// Running first, then the newest started; ties by bot and id, so pages
/// are stable.
fn order(a: &Value, b: &Value) -> Ordering {
    running(b)
        .cmp(&running(a))
        .then_with(|| text(b, "started_at").cmp(text(a, "started_at")))
        .then_with(|| text(a, "bot_id").cmp(text(b, "bot_id")))
        .then_with(|| text(a, "id").cmp(text(b, "id")))
}

/// Where a row sits in that order, as `next_before` carries it.
fn cursor(row: &Value) -> String {
    let rank = if running(row) { 0 } else { 1 };
    format!(
        "{rank}|{}|{}|{}",
        text(row, "started_at"),
        text(row, "bot_id"),
        text(row, "id")
    )
}

/// A row as the cursor that would name it, for comparing against one.
fn as_cursor_row(cursor: &str) -> Option<Value> {
    let mut parts = cursor.splitn(4, '|');
    let rank = parts.next()?;
    let started_at = parts.next()?;
    let bot_id = parts.next()?;
    let id = parts.next()?;
    let status = if rank == "0" { "running" } else { "done" };
    Some(json!({ "status": status, "started_at": started_at, "bot_id": bot_id, "id": id }))
}

fn matches(row: &Value, filter: &Filter) -> bool {
    let status = match filter.status.as_deref().unwrap_or("all") {
        "running" => running(row),
        "failed" => text(row, "status") == "failed",
        _ => true,
    };
    let query = filter.query.as_deref().map(str::trim).unwrap_or_default();
    let found = query.is_empty() || {
        let query = query.to_lowercase();
        text(row, "command").to_lowercase().contains(&query)
            || text(row, "description").to_lowercase().contains(&query)
    };
    status && found
}

/// The `commands` answer: one page of every bot's commands, and the
/// summary per computer, which counts every command of the bots named,
/// whatever the status and query.
pub async fn list(app: &Arc<AppState>, filter: Filter) -> anyhow::Result<Value> {
    let read = app.clone();
    let asked = filter.clone();
    let (bots, here) = tokio::task::spawn_blocking(move || -> anyhow::Result<_> {
        let bots = bots(&read, &asked)?;
        let here = local(&read, &bots)?;
        Ok((bots, here))
    })
    .await??;
    let mut by_peer: Vec<(String, Vec<Bot>)> = Vec::new();
    for bot in bots.into_iter().filter(Bot::is_linked) {
        let Some(peer) = bot.peer_id.clone() else {
            continue;
        };
        match by_peer.iter_mut().find(|(p, _)| *p == peer) {
            Some((_, bots)) => bots.push(bot),
            None => by_peer.push((peer, vec![bot])),
        }
    }
    let asks = by_peer
        .into_iter()
        .map(|(peer, bots)| async move { linked(app, &peer, bots).await });
    let mut parts = vec![here];
    parts.extend(futures::future::join_all(asks).await);
    Ok(answer(parts, &filter))
}

fn answer(parts: Vec<Part>, filter: &Filter) -> Value {
    let summary: Vec<Value> = parts
        .iter()
        .map(|p| {
            let mut s = json!({
                "daemon_id": p.daemon_id, "computer_name": p.computer_name,
                "running": p.rows.iter().filter(|r| running(r)).count(),
                "failed_today": p.rows.iter().filter(|r| failed_today(r)).count(),
            });
            if p.stale {
                s["stale"] = json!(true);
            }
            s
        })
        .collect();
    let mut rows: Vec<Value> = parts
        .into_iter()
        .flat_map(|p| p.rows)
        .filter(|r| matches(r, filter))
        .collect();
    rows.sort_by(order);
    if let Some(before) = filter.before.as_deref().and_then(as_cursor_row) {
        rows.retain(|r| order(r, &before) == Ordering::Greater);
    }
    let limit = filter.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
    let more = rows.len() > limit;
    rows.truncate(limit);
    let mut out = json!({ "type": "commands", "commands": rows, "summary": summary });
    if more {
        if let Some(last) = out["commands"].as_array().and_then(|r| r.last()) {
            out["next_before"] = json!(cursor(last));
        }
    }
    out
}

#[cfg(test)]
#[path = "commands_tests.rs"]
mod tests;
