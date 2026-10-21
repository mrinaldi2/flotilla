//! What a bot's command list adds to its transcript (0.24.0): the card each
//! command's turn was for, and the commands the owner stopped.

use std::collections::HashMap;

use bus::Bot;

use crate::app::AppState;

use super::commands::{Command, CommandStatus};
use super::model::{ChatItem, ChatTurn, Trigger};

/// Each command's bus task, by the command's id: the task of the message
/// that started its turn.
pub(super) fn tasks_of(turns: &[ChatTurn]) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for turn in turns {
        let Trigger::Bus {
            task_id: Some(task),
            ..
        } = &turn.trigger
        else {
            continue;
        };
        for item in &turn.items {
            if let ChatItem::Step(step) = item {
                out.insert(step.id.clone(), task.clone());
            }
        }
    }
    out
}

/// Marks the card of each command whose task names one, and the commands
/// the owner stopped: stopped, from when the owner stopped them, whatever
/// the transcript says yet.
pub(super) fn mark(
    app: &AppState,
    bot: &Bot,
    commands: &mut [Command],
    tasks: &HashMap<String, String>,
) {
    let stops = app.db.command_stops(&bot.id).unwrap_or_default();
    let mut cards: HashMap<&str, Option<String>> = HashMap::new();
    for command in commands {
        if let Some(task) = tasks.get(&command.id) {
            command.bus_task = Some(task.clone());
            command.item_id = cards
                .entry(task.as_str())
                .or_insert_with(|| app.db.task_card(task).ok().flatten())
                .clone();
        }
        if let Some(at) = stops.get(&command.id) {
            command.stopped_by_owner = true;
            if command.status == CommandStatus::Running {
                command.status = CommandStatus::Stopped;
                command.ended_at = Some(*at);
            }
        }
    }
}
