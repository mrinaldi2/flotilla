//! The owner's Stop on a bot's command (0.24.0, `stop_command`): owner
//! proof required, the command's processes ended, the command marked
//! stopped by the owner, the bot told, and a finished command refused.

use crate::common;

use std::time::Duration;

use common::commands::{bash, output_file, result, started, transcript};
use common::peers::project;
use common::tasks::drain_until;
use common::*;
use serde_json::{json, Value};

/// A process writing to `output` as a background command does, leading
/// its own process group.
fn serve(output: &std::path::Path) -> std::process::Child {
    use std::os::unix::process::CommandExt;
    let file = std::fs::OpenOptions::new()
        .append(true)
        .open(output)
        .expect("output");
    std::process::Command::new("sh")
        .args(["-c", "sleep 300; echo done"])
        .stdout(file)
        .process_group(0)
        .spawn()
        .expect("spawn")
}

#[tokio::test]
async fn only_the_owner_stops_a_running_command_and_the_bot_is_told() {
    let d = spawn_daemon_with(|cfg| cfg.user_home = cfg.home.join("user")).await;
    let mut c = WsClient::connect(&d).await;
    let pid = project(&mut c, "Game").await;
    let bot = create_bot(&mut c, &pid, "builder").await;
    let bot_id = bot["id"].as_str().expect("id").to_string();
    let output = output_file(&d, "bx1");
    let mut child = serve(&output);
    transcript(
        &d,
        &bot_id,
        &[
            bash("t1", "cargo test", false, "2026-10-01T10:00:00Z"),
            result("t1", "test result: ok", false, "2026-10-01T10:00:09Z"),
            bash("b1", "./serve.sh --port 8080", true, "2026-10-01T09:00:00Z"),
            started("b1", "bx1", &output, "2026-10-01T09:00:01Z"),
        ],
    );
    let stop = json!({"type": "stop_command", "bot_id": bot_id, "command_id": "b1"});

    // The owner token file is one a bot can read: never the owner's hands.
    let mut token = WsClient::connect_owner_token(&d).await;
    let refused = token.request(stop.clone()).await;
    assert_eq!(refused["code"], "forbidden", "{refused}");
    assert!(child.try_wait().expect("child").is_none(), "still running");

    let stopped = c.request(stop.clone()).await;
    assert_eq!(stopped["type"], "ok", "{stopped}");
    let row = &stopped["command"];
    assert_eq!(row["status"], "stopped", "{row}");
    assert_eq!(row["stopped_by_owner"], true);
    assert_eq!(row["bot_id"], bot_id.as_str());

    // Its process is gone.
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let status = loop {
        if let Some(status) = child.try_wait().expect("child") {
            break status;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "the command still runs"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    };
    assert!(!status.success(), "ended by a signal: {status:?}");

    // The list says so, whatever the transcript says yet.
    let listed = c
        .request(json!({"type": "list_commands", "bot_id": bot_id}))
        .await;
    let serve = &listed["commands"]
        .as_array()
        .expect("commands")
        .iter()
        .find(|c| c["id"] == "b1")
        .cloned()
        .expect("the stopped command");
    assert_eq!(serve["status"], "stopped");
    assert_eq!(serve["stopped_by_owner"], true);

    // The bot is told, by name of the command, not to run it again blindly.
    let token = d.app.secrets.bot_token(&bot_id).expect("bot token");
    let mut mcp = McpClient::new(&d, &token);
    let notes = drain_until(&mut mcp, "The owner stopped your command").await;
    let note = notes
        .iter()
        .filter_map(|n| n["body"].as_str())
        .find(|b| b.contains("The owner stopped your command"))
        .expect("the bot is told");
    assert!(note.contains("`./serve.sh --port 8080`"), "{note}");
    assert!(note.contains("without asking"), "{note}");

    // A finished command can't be stopped, nor one stopped already.
    let again = c.request(stop).await;
    assert_eq!(again["code"], "conflict", "{again}");
    let done = c
        .request(json!({"type": "stop_command", "bot_id": bot_id, "command_id": "t1"}))
        .await;
    assert_eq!(done["code"], "conflict", "{done}");
    let missing = c
        .request(json!({"type": "stop_command", "bot_id": bot_id, "command_id": "nope"}))
        .await;
    assert_eq!(missing["code"], "not_found", "{missing}");
}

#[tokio::test]
async fn a_linked_bots_command_is_stopped_on_its_computer() {
    let mut t = common::peers::team().await;
    let output = output_file(&t.win, "wx1");
    transcript(
        &t.win,
        &t.windev_id,
        &[
            bash("w1", "msbuild player.sln", true, "2026-10-01T09:00:00Z"),
            started("w1", "wx1", &output, "2026-10-01T09:00:01Z"),
        ],
    );
    let stopped = t
        .mac_client
        .request(json!({"type": "stop_command", "bot_id": t.linked_windev, "command_id": "w1"}))
        .await;
    assert_eq!(stopped["type"], "ok", "{stopped}");
    assert_eq!(stopped["command"]["status"], "stopped");
    assert_eq!(stopped["command"]["bot_id"], t.linked_windev.as_str());
    let stops = t.win.app.db.command_stops(&t.windev_id).unwrap();
    assert!(stops.contains_key("w1"), "recorded on the bot's computer");
    let again: Value = t
        .mac_client
        .request(json!({"type": "stop_command", "bot_id": t.linked_windev, "command_id": "w1"}))
        .await;
    assert_eq!(again["code"], "conflict", "{again}");
}
