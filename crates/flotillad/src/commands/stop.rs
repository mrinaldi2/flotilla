//! The owner's Stop on a bot's command (0.24.0, `stop_command`, owner
//! only). It ends a running background command's processes, records it as
//! stopped by the owner, and tells the bot, naming the command and its card
//! or task, so the bot doesn't run it again blindly. A linked bot's command
//! is stopped on its own computer, which tells it.

use std::path::Path;
use std::sync::Arc;

use bus::Bot;
use chrono::Utc;
use serde_json::{json, Value};

use crate::app::AppState;
use crate::chat::commands::{Command, CommandStatus};
use crate::decisions::{conflict, invalid, not_found};

/// A command's first line, short enough to quote.
fn quoted(command: &str) -> String {
    let line = command.lines().next().unwrap_or_default().trim();
    let short: String = line.chars().take(120).collect();
    match short.len() < line.len() {
        true => format!("{short}…"),
        false => short,
    }
}

/// What the bot is told.
fn message(command: &Command, task: Option<&str>) -> String {
    let what = match (&command.item_id, task) {
        (Some(card), _) => format!(", which you ran for card {card}"),
        (None, Some(task)) => format!(", which you ran for task {task}"),
        (None, None) => String::new(),
    };
    format!(
        "The owner stopped your command `{}`{what}. Don't run it again without asking the \
         owner first: say what you need it for, or carry on without it.",
        quoted(&command.command)
    )
}

/// The bot's command `command_id`, running in the background.
fn running(app: &AppState, bot: &Bot, command_id: &str) -> anyhow::Result<Command> {
    let command = app
        .chat
        .commands(app, bot, usize::MAX)?
        .into_iter()
        .find(|c| c.id == command_id)
        .ok_or_else(|| not_found(format!("{} has no command {command_id}", bot.name)))?;
    if command.status != CommandStatus::Running {
        let status = serde_json::to_value(command.status)?;
        return Err(conflict(format!(
            "that command already ended ({})",
            status.as_str().unwrap_or_default()
        )));
    }
    if !command.background {
        return Err(invalid(
            "only a background command can be stopped: a foreground one ends with the bot's turn",
        ));
    }
    Ok(command)
}

/// Stops one of this computer's bots' commands as `by`, and returns the
/// command as the list now shows it.
pub fn here(app: &AppState, bot: &Bot, command_id: &str, by: &str) -> anyhow::Result<Command> {
    let command = running(app, bot, command_id)?;
    let keep: Vec<u32> = app
        .supervisor
        .session_roots()
        .root_of(&bot.id)
        .map(|root| root.pid)
        .into_iter()
        .collect();
    let ended = match &command.output_file {
        Some(file) => super::kill::end(Path::new(file), &keep)?,
        None => 0,
    };
    app.db
        .record_command_stop(&bot.id, &command.id, &command.command, by, Utc::now())?;
    tracing::info!(bot_id = %bot.id, command_id, ended, by, "the owner stopped a command");
    let sender = crate::messaging::user_sender();
    let body = message(&command, command.bus_task.as_deref());
    let dm = crate::messaging::Dm::new(&bot.id, &sender, bus::MessageKind::Note, &body);
    if let Err(e) = crate::messaging::send_dm(&app.db, &app.events, dm) {
        tracing::warn!(error = %e, "the stopped command's note failed");
    }
    if let Some(card) = &command.item_id {
        let note = format!(
            "The owner stopped `{}` ({}).",
            quoted(&command.command),
            bot.name
        );
        let _ = app
            .db
            .add_item_comment(card, &note, None, &crate::actor::Actor::User);
    }
    super::watch::changed(app, bot);
    app.chat
        .commands(app, bot, usize::MAX)?
        .into_iter()
        .find(|c| c.id == command.id)
        .ok_or_else(|| not_found("the stopped command is gone"))
}

/// `stop_command {bot_id, command_id}`: the owner's Stop, as the row the
/// list shows after it.
pub async fn stop(
    app: &Arc<AppState>,
    bot_id: &str,
    command_id: &str,
    by: &str,
) -> anyhow::Result<Value> {
    let bot = app
        .db
        .get_live_bot(bot_id)?
        .ok_or_else(|| not_found(format!("no bot {bot_id}")))?;
    if let (Some(peer_id), Some(remote)) = (&bot.peer_id, &bot.remote_bot_id) {
        // Its computer's refusal keeps its code: a finished command is a
        // conflict there too.
        let frame = json!({ "type": "stop_command", "bot_id": remote, "command_id": command_id });
        let result = app.peers.request(peer_id, frame).await?;
        let daemon = app
            .db
            .get_peer(peer_id)?
            .and_then(|p| p.daemon_id)
            .unwrap_or_default();
        return Ok(super::row(result["command"].clone(), &bot, &daemon));
    }
    let (app, by, command_id) = (app.clone(), by.to_string(), command_id.to_string());
    tokio::task::spawn_blocking(move || {
        let command = here(&app, &bot, &command_id, &by)?;
        Ok(super::row(
            serde_json::to_value(command)?,
            &bot,
            &app.db.daemon_id()?,
        ))
    })
    .await?
}

/// A linked computer asks to stop one of this computer's bots' commands:
/// the owner's Stop there, for a bot linked to it.
pub fn serve(app: &AppState, peer: &bus::Peer, bot: &Bot, frame: &Value) -> anyhow::Result<Value> {
    let command_id = frame["command_id"].as_str().unwrap_or_default();
    let by = format!("peer:{}", peer.id);
    // Its refusal reaches the asking computer with its code.
    let command =
        here(app, bot, command_id, &by).map_err(|e| match crate::decisions::error_code(&e) {
            Some(code) => anyhow::Error::new(crate::peer::Refusal {
                code,
                message: format!("{e:#}"),
            }),
            None => e,
        })?;
    Ok(json!({ "command": command }))
}
