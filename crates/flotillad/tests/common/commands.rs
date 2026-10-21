//! A bot's commands as its runtime records them (0.24.0): transcript
//! records for foreground and background commands, a background command's
//! output file, and the transcript written where the daemon reads it.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use super::transcripts::transcript_dir;
use super::TestDaemon;

pub fn bash(id: &str, command: &str, background: bool, at: &str) -> Value {
    json!({"type": "assistant", "uuid": format!("a-{id}"), "sessionId": "s1", "timestamp": at,
           "message": {"content": [{"type": "tool_use", "id": id, "name": "Bash",
               "input": {"command": command, "description": format!("{command} for the test"),
                         "run_in_background": background}}]}})
}

pub fn result(id: &str, text: &str, error: bool, at: &str) -> Value {
    json!({"type": "user", "uuid": format!("r-{id}"), "sessionId": "s1", "timestamp": at,
           "message": {"content": [{"type": "tool_result", "tool_use_id": id,
               "content": text, "is_error": error}]}})
}

/// The runtime's answer to a background command, writing to `output`.
pub fn started(id: &str, task: &str, output: &Path, at: &str) -> Value {
    let text = format!(
        "Command running in background with ID: {task}. Output is being written to: {}. You will be notified when it completes.",
        output.display()
    );
    result(id, &text, false, at)
}

/// A background command's output file, as the runtime names it.
pub fn output_file(d: &TestDaemon, task: &str) -> PathBuf {
    let tasks = d.app.cfg.home.join("tasks");
    std::fs::create_dir_all(&tasks).expect("tasks");
    let output = tasks.join(format!("{task}.output"));
    std::fs::write(&output, "serving on :8080\n").expect("output");
    output
}

/// Writes the bot's Claude Code transcript; returns its path.
pub fn transcript(d: &TestDaemon, bot_id: &str, records: &[Value]) -> PathBuf {
    let bot = d.app.db.get_bot(bot_id).expect("db").expect("bot");
    let dir = transcript_dir(&d.app.cfg.user_home, &bot.workspace_path);
    std::fs::create_dir_all(&dir).expect("dir");
    let lines: Vec<String> = records.iter().map(Value::to_string).collect();
    let path = dir.join("session.jsonl");
    std::fs::write(&path, lines.join("\n") + "\n").expect("write");
    path
}

pub fn append(path: &Path, record: &Value) {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(path)
        .expect("transcript");
    writeln!(file, "{record}").expect("append");
}
