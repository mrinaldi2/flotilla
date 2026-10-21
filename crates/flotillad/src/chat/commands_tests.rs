//! The command log read from transcript lines: foreground, background and
//! stopped commands.

use super::*;
use serde_json::json;

fn line(value: Value) -> String {
    value.to_string()
}

fn call(id: &str, input: Value) -> String {
    line(
        json!({"type": "assistant", "sessionId": "s1", "timestamp": "2026-10-01T10:00:00Z",
        "message": {"content": [{"type": "tool_use", "id": id, "name": "Bash", "input": input}]}}),
    )
}

fn result(id: &str, text: &str, error: bool) -> String {
    line(
        json!({"type": "user", "sessionId": "s1", "timestamp": "2026-10-01T10:00:05Z",
        "message": {"content": [{"type": "tool_result", "tool_use_id": id, "content": text, "is_error": error}]}}),
    )
}

#[test]
fn follows_foreground_commands_to_their_result() {
    let mut log = CommandLog::default();
    log.push_line(&call(
        "c1",
        json!({"command": "cargo test", "description": "Run tests"}),
    ));
    assert_eq!(log.list(true, 10)[0].status, CommandStatus::Running);
    // An idle bot is running nothing in the foreground.
    assert_eq!(log.list(false, 10)[0].status, CommandStatus::Stopped);
    log.push_line(&result("c1", "test result: ok", false));
    let done = &log.list(false, 10)[0];
    assert_eq!(done.status, CommandStatus::Done);
    assert_eq!(done.output.as_deref(), Some("test result: ok"));
    log.push_line(&call("c2", json!({"command": "false"})));
    log.push_line(&result("c2", "exit 1", true));
    assert_eq!(log.list(false, 10)[0].status, CommandStatus::Failed);
}

#[test]
fn follows_background_commands_until_notified_or_stopped() {
    let mut log = CommandLog::default();
    log.push_line(&call(
        "b1",
        json!({"command": "./build.sh", "run_in_background": true}),
    ));
    log.push_line(&result(
        "b1",
        "Command running in background with ID: bx1. Output is being written to: /tmp/s/tasks/bx1.output. You will be notified when it completes.",
        false,
    ));
    let running = &log.list(false, 10)[0];
    assert_eq!(running.status, CommandStatus::Running);
    assert_eq!(running.task_id.as_deref(), Some("bx1"));
    assert_eq!(
        running.output_file.as_deref(),
        Some("/tmp/s/tasks/bx1.output")
    );

    log.push_line(&line(json!({"type": "queue-operation", "sessionId": "s1",
        "timestamp": "2026-10-01T10:05:00Z",
        "content": "<task-notification>\n<task-id>bx1</task-id>\n<tool-use-id>b1</tool-use-id>\n<status>completed</status>\n<summary>Background command \"build\" completed (exit code 2)</summary>\n</task-notification>"})));
    let failed = &log.list(false, 10)[0];
    assert_eq!(failed.status, CommandStatus::Failed);
    assert_eq!(failed.exit_code, Some(2));

    log.push_line(&call(
        "b2",
        json!({"command": "./serve.sh", "run_in_background": true}),
    ));
    log.push_line(&result("b2", "Command running in background with ID: bx2. Output is being written to: /tmp/s/tasks/bx2.output.", false));
    log.push_line(&line(json!({"type": "assistant", "sessionId": "s1", "timestamp": "2026-10-01T10:06:00Z",
        "message": {"content": [{"type": "tool_use", "id": "k1", "name": "TaskStop", "input": {"task_id": "bx2"}}]}})));
    log.push_line(&result("k1", "stopped", false));
    assert_eq!(log.list(false, 10)[0].status, CommandStatus::Stopped);
}

#[test]
fn a_command_from_an_earlier_session_is_no_longer_running() {
    let mut log = CommandLog::default();
    log.push_line(&call(
        "b1",
        json!({"command": "sleep 99", "run_in_background": true}),
    ));
    log.push_line(&result(
        "b1",
        "Command running in background with ID: bx1.",
        false,
    ));
    log.push_line(&line(
        json!({"type": "user", "sessionId": "s2", "timestamp": "2026-10-01T11:00:00Z",
        "message": {"content": "hello"}}),
    ));
    assert_eq!(log.list(true, 10)[0].status, CommandStatus::Stopped);
}
