//! The commands a bot runs, read from its transcript: each `Bash` call, from
//! the moment it starts to its result, and for one run in the background,
//! until the runtime reports it finished (`<task-notification>`) or the bot
//! stops it (`TaskStop`). Codex sessions record their commands in the same
//! shape.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;

/// Characters of a command's output kept for the list.
pub const OUTPUT_TAIL_CHARS: usize = 4_000;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CommandStatus {
    Running,
    Done,
    Failed,
    /// Stopped by the bot, or by its session ending under it.
    Stopped,
}

#[derive(Debug, Clone, Serialize)]
pub struct Command {
    /// The tool call's id.
    pub id: String,
    pub command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub background: bool,
    pub status: CommandStatus,
    pub started_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i64>,
    /// The runtime's id for a background command.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    /// Where a background command's output is being written.
    #[serde(skip)]
    pub output_file: Option<String>,
    /// The end of what the command printed, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    /// The card its turn was for, when known (0.24.0).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item_id: Option<String>,
    /// The owner stopped it from the Commands page (0.24.0).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub stopped_by_owner: bool,
    /// The bus task its turn answered, when one did.
    #[serde(skip)]
    pub bus_task: Option<String>,
    #[serde(skip)]
    session: Option<String>,
}

/// Every command in a transcript, in the order they started.
#[derive(Debug, Default)]
pub struct CommandLog {
    commands: Vec<Command>,
    by_call: HashMap<String, usize>,
    by_task: HashMap<String, usize>,
    /// A stop call's id → the background command it stops.
    stops: HashMap<String, String>,
    last_session: Option<String>,
    /// Bumped whenever a command starts, ends or is stopped, so a watcher
    /// can tell the list changed (0.24.0).
    changes: u64,
}

fn timestamp(record: &Value) -> DateTime<Utc> {
    record["timestamp"]
        .as_str()
        .and_then(|t| DateTime::parse_from_rfc3339(t).ok())
        .map_or_else(Utc::now, |t| t.with_timezone(&Utc))
}

fn tail(text: &str) -> String {
    let count = text.chars().count();
    text.chars()
        .skip(count.saturating_sub(OUTPUT_TAIL_CHARS))
        .collect()
}

/// The text of a tool result, whether a string or a list of blocks.
fn result_text(block: &Value) -> String {
    match &block["content"] {
        Value::String(text) => text.clone(),
        Value::Array(parts) => parts
            .iter()
            .filter_map(|p| p["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

/// The text between `<tag>` and `</tag>`.
fn tag<'a>(text: &'a str, name: &str) -> Option<&'a str> {
    let open = format!("<{name}>");
    let start = text.find(&open)? + open.len();
    let end = text[start..].find(&format!("</{name}>"))? + start;
    Some(text[start..end].trim())
}

/// "(exit code 2)" in a notification's summary.
fn exit_code(summary: &str) -> Option<i64> {
    let start = summary.find("exit code ")? + "exit code ".len();
    let digits: String = summary[start..]
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '-')
        .collect();
    digits.parse().ok()
}

/// The end of a background command's output file. Only a runtime's own
/// task output files are read: `…/tasks/<id>.output`.
pub fn read_tail(path: &std::path::Path) -> Option<String> {
    use std::io::{Read, Seek, SeekFrom};
    let named = path.extension().is_some_and(|e| e == "output")
        && path
            .parent()
            .and_then(|dir| dir.file_name())
            .is_some_and(|dir| dir == "tasks");
    if !named {
        return None;
    }
    let mut file = std::fs::File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    let window = (OUTPUT_TAIL_CHARS * 4) as u64;
    file.seek(SeekFrom::Start(len.saturating_sub(window)))
        .ok()?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).ok()?;
    Some(tail(&String::from_utf8_lossy(&bytes)))
}

impl CommandLog {
    /// An empty log whose change count goes on from `changes`.
    pub fn after(changes: u64) -> Self {
        Self {
            changes: changes + 1,
            ..Self::default()
        }
    }

    /// Folds one transcript line in.
    pub fn push_line(&mut self, line: &str) {
        let Ok(record) = serde_json::from_str::<Value>(line) else {
            return;
        };
        if let Some(session) = record["sessionId"].as_str() {
            self.last_session = Some(session.to_string());
        }
        if line.contains("<task-notification>") {
            self.notification(&record);
        }
        let Some(blocks) = record["message"]["content"].as_array() else {
            return;
        };
        let at = timestamp(&record);
        for block in blocks {
            match block["type"].as_str() {
                Some("tool_use") => self.call(block, at, record["sessionId"].as_str()),
                Some("tool_result") => self.result(block, at),
                _ => {}
            }
        }
    }

    fn call(&mut self, block: &Value, at: DateTime<Utc>, session: Option<&str>) {
        let (Some(id), Some(name)) = (block["id"].as_str(), block["name"].as_str()) else {
            return;
        };
        let input = &block["input"];
        match name {
            "Bash" => {
                self.by_call.insert(id.to_string(), self.commands.len());
                self.commands.push(Command {
                    id: id.to_string(),
                    command: input["command"].as_str().unwrap_or_default().to_string(),
                    description: input["description"].as_str().map(str::to_string),
                    background: input["run_in_background"].as_bool().unwrap_or(false),
                    status: CommandStatus::Running,
                    started_at: at,
                    ended_at: None,
                    exit_code: None,
                    task_id: None,
                    output_file: None,
                    output: None,
                    item_id: None,
                    stopped_by_owner: false,
                    bus_task: None,
                    session: session.map(str::to_string),
                });
                self.changes += 1;
            }
            "TaskStop" | "KillShell" | "KillBash" => {
                let task = ["task_id", "shell_id", "bash_id"]
                    .iter()
                    .find_map(|key| input[*key].as_str());
                if let Some(task) = task {
                    self.stops.insert(id.to_string(), task.to_string());
                }
            }
            _ => {}
        }
    }

    fn result(&mut self, block: &Value, at: DateTime<Utc>) {
        let Some(call) = block["tool_use_id"].as_str() else {
            return;
        };
        if let Some(task) = self.stops.remove(call) {
            if let Some(command) = self.by_task.get(&task).map(|&i| &mut self.commands[i]) {
                if command.status == CommandStatus::Running {
                    command.status = CommandStatus::Stopped;
                    command.ended_at = Some(at);
                }
            }
            self.changes += 1;
            return;
        }
        let Some(&index) = self.by_call.get(call) else {
            return;
        };
        let text = result_text(block);
        self.changes += 1;
        let command = &mut self.commands[index];
        if let Some(rest) = text.split("running in background with ID: ").nth(1) {
            let task: String = rest
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
                .collect();
            command.background = true;
            command.output_file = rest
                .split("Output is being written to: ")
                .nth(1)
                .and_then(|path| path.split_whitespace().next())
                .map(|path| path.trim_end_matches('.').to_string());
            self.by_task.insert(task.clone(), index);
            command.task_id = Some(task);
            return;
        }
        command.status = if block["is_error"].as_bool() == Some(true) {
            CommandStatus::Failed
        } else {
            CommandStatus::Done
        };
        command.ended_at = Some(at);
        command.output = Some(tail(&text));
    }

    /// A background command's end, as the runtime reports it.
    fn notification(&mut self, record: &Value) {
        let text = [
            &record["content"],
            &record["attachment"]["prompt"],
            &record["message"]["content"],
        ]
        .into_iter()
        .find_map(|v| v.as_str().filter(|t| t.contains("<task-notification>")))
        .unwrap_or_default();
        let index = tag(text, "tool-use-id")
            .and_then(|call| self.by_call.get(call))
            .or_else(|| tag(text, "task-id").and_then(|task| self.by_task.get(task)))
            .copied();
        let (Some(index), Some(status)) = (index, tag(text, "status")) else {
            return;
        };
        if self.commands[index].status != CommandStatus::Running {
            return;
        }
        self.changes += 1;
        let command = &mut self.commands[index];
        command.exit_code = tag(text, "summary").and_then(exit_code);
        command.status = match status {
            "completed" if command.exit_code.unwrap_or(0) == 0 => CommandStatus::Done,
            "completed" | "failed" => CommandStatus::Failed,
            _ => CommandStatus::Stopped,
        };
        command.ended_at = Some(timestamp(record));
    }

    /// How many times the log changed: a command started, ended or stopped.
    pub fn changes(&self) -> u64 {
        self.changes
    }

    /// The output files of background commands still running, whose growth
    /// is a command printing.
    pub fn running_outputs(&self) -> Vec<String> {
        self.commands
            .iter()
            .filter(|c| c.status == CommandStatus::Running && c.background)
            .filter_map(|c| c.output_file.clone())
            .collect()
    }

    /// When a command mentioning `needle` (a file's path) first ran.
    pub fn first_mention(&self, needle: &str) -> Option<DateTime<Utc>> {
        self.commands
            .iter()
            .find(|command| command.command.contains(needle))
            .map(|command| command.started_at)
    }

    /// Every command, the running ones first, then the newest. A command
    /// still running in an earlier session stopped with it, and a foreground
    /// one cannot be running while the bot is idle.
    pub fn list(&self, busy: bool, limit: usize) -> Vec<Command> {
        let mut out: Vec<Command> = self
            .commands
            .iter()
            .rev()
            .map(|command| {
                let mut command = command.clone();
                let old_session = command.session.is_some() && command.session != self.last_session;
                if command.status == CommandStatus::Running
                    && (old_session || (!command.background && !busy))
                {
                    command.status = CommandStatus::Stopped;
                }
                command
            })
            .collect();
        out.sort_by_key(|c| c.status != CommandStatus::Running);
        out.truncate(limit);
        out
    }
}

#[cfg(test)]
#[path = "commands_tests.rs"]
mod tests;
