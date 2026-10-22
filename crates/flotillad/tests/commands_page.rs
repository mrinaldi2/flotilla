//! The Commands page (0.24.0, `commands_v1`): every bot's commands in one
//! list across projects, its filters and pages, a push when a bot's
//! commands change, and a linked computer's bots read from there.

use crate::common;

use std::path::PathBuf;

use common::commands::{append, bash, output_file, result, started, transcript};
use common::peers::{project, team};
use common::*;
use serde_json::{json, Value};

/// A transcript time this many seconds from now: after the test bot's
/// runtime started, as a real command's is (0.24.0).
fn now_plus(secs: i64) -> String {
    (chrono::Utc::now() + chrono::Duration::seconds(secs)).to_rfc3339()
}

fn id(v: &Value) -> String {
    v["id"].as_str().expect("id").to_string()
}

struct Setup {
    d: TestDaemon,
    c: WsClient,
    builder: String,
    tester: String,
    game: String,
    site: String,
    path: PathBuf,
}

/// Two projects: a builder running a server in the background after a
/// test run, and a tester whose build failed today.
async fn setup() -> Setup {
    let d = spawn_daemon_with(|cfg| cfg.user_home = cfg.home.join("user")).await;
    let mut c = WsClient::connect(&d).await;
    let game = project(&mut c, "Game").await;
    let site = project(&mut c, "Site").await;
    let builder = id(&create_bot(&mut c, &game, "builder").await);
    let tester = id(&create_bot(&mut c, &site, "tester").await);
    let output = output_file(&d, "bx1");
    let path = transcript(
        &d,
        &builder,
        &[
            bash("t1", "cargo test", false, &now_plus(60)),
            result("t1", "test result: ok", false, &now_plus(69)),
            bash("b1", "./serve.sh", true, &now_plus(0)),
            started("b1", "bx1", &output, &now_plus(1)),
        ],
    );
    let today = now_plus(120);
    transcript(
        &d,
        &tester,
        &[
            bash("p1", "pnpm build", false, &today),
            result("p1", "error TS2304", true, &today),
            bash("g1", "git status", false, "2026-09-30T10:00:00Z"),
            result("g1", "clean", false, "2026-09-30T10:00:01Z"),
        ],
    );
    Setup {
        d,
        c,
        builder,
        tester,
        game,
        site,
        path,
    }
}

fn commands(v: &Value) -> Vec<String> {
    assert_eq!(v["type"], "commands", "{v}");
    v["commands"]
        .as_array()
        .expect("commands")
        .iter()
        .map(|c| c["command"].as_str().expect("command").to_string())
        .collect()
}

#[tokio::test]
async fn every_bots_commands_in_one_list_with_filters_and_pages() {
    let mut s = setup().await;
    let all = s.c.request(json!({"type": "list_commands"})).await;
    assert_eq!(
        commands(&all),
        ["./serve.sh", "pnpm build", "cargo test", "git status"],
        "running first, then the newest"
    );
    let serve = &all["commands"][0];
    assert_eq!(serve["status"], "running");
    assert_eq!(serve["background"], true);
    assert_eq!(serve["bot_id"], s.builder.as_str());
    assert_eq!(serve["bot_name"], "builder");
    assert_eq!(serve["project_id"], s.game.as_str());
    let here = s.d.app.db.daemon_id().unwrap();
    assert_eq!(serve["daemon_id"], here.as_str());
    assert!(serve["output"]
        .as_str()
        .unwrap()
        .contains("serving on :8080"));
    let summary = all["summary"].as_array().expect("summary");
    assert_eq!(summary.len(), 1, "{summary:?}");
    assert_eq!(summary[0]["daemon_id"], here.as_str());
    assert_eq!(summary[0]["running"], 1);
    assert_eq!(summary[0]["failed_today"], 1);

    let site =
        s.c.request(json!({"type": "list_commands", "project_id": s.site}))
            .await;
    assert_eq!(commands(&site), ["pnpm build", "git status"]);
    let builder =
        s.c.request(json!({"type": "list_commands", "bot_id": s.builder}))
            .await;
    assert_eq!(commands(&builder), ["./serve.sh", "cargo test"]);
    let running =
        s.c.request(json!({"type": "list_commands", "status": "running"}))
            .await;
    assert_eq!(commands(&running), ["./serve.sh"]);
    let failed =
        s.c.request(json!({"type": "list_commands", "status": "failed"}))
            .await;
    assert_eq!(commands(&failed), ["pnpm build"]);
    assert_eq!(failed["commands"][0]["bot_id"], s.tester.as_str());
    let found =
        s.c.request(json!({"type": "list_commands", "query": "CARGO"}))
            .await;
    assert_eq!(commands(&found), ["cargo test"]);
    let bad =
        s.c.request(json!({"type": "list_commands", "status": "stopped"}))
            .await;
    assert_eq!(bad["code"], "invalid_request", "{bad}");

    let first =
        s.c.request(json!({"type": "list_commands", "limit": 3}))
            .await;
    assert_eq!(commands(&first), ["./serve.sh", "pnpm build", "cargo test"]);
    let before = first["next_before"].as_str().expect("a next page");
    let next =
        s.c.request(json!({"type": "list_commands", "limit": 3, "before": before}))
            .await;
    assert_eq!(commands(&next), ["git status"]);
    assert!(next.get("next_before").is_none(), "{next}");

    // Reading takes `read`, and the owner token holds it.
    let mut token = WsClient::connect_owner_token(&s.d).await;
    let read = token.request(json!({"type": "list_commands"})).await;
    assert_eq!(commands(&read).len(), 4);
}

#[tokio::test]
async fn a_bots_new_command_is_pushed_instead_of_polled() {
    let mut s = setup().await;
    s.c.request(json!({"type": "list_commands"})).await;
    // Whatever the first reads announce is over before this client
    // listens: what it hears comes from the new command.
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    let mut c = WsClient::connect(&s.d).await;
    let at = chrono::Utc::now().to_rfc3339();
    append(&s.path, &bash("n1", "cargo clippy", false, &at));
    let builder = s.builder.clone();
    let pushed = c
        .wait_for(|f| f["type"] == "commands_changed" && f["bot_id"] == builder.as_str())
        .await;
    assert_eq!(pushed["project_id"], s.game.as_str());
    assert_eq!(
        pushed["daemon_id"],
        s.d.app.db.daemon_id().unwrap().as_str()
    );
    let all = s.c.request(json!({"type": "list_commands"})).await;
    assert!(commands(&all).contains(&"cargo clippy".to_string()));
}

#[tokio::test]
async fn a_linked_computers_bots_are_read_from_there_and_pushed_here() {
    let mut t = team().await;
    let output = output_file(&t.win, "wx1");
    let path = transcript(
        &t.win,
        &t.windev_id,
        &[
            bash("w1", "msbuild player.sln", true, &now_plus(0)),
            started("w1", "wx1", &output, &now_plus(1)),
        ],
    );
    let listed = t
        .mac_client
        .request(json!({"type": "list_commands", "bot_id": t.linked_windev}))
        .await;
    assert_eq!(commands(&listed), ["msbuild player.sln"]);
    let row = &listed["commands"][0];
    assert_eq!(row["bot_id"], t.linked_windev.as_str());
    let win = t.win.app.db.daemon_id().unwrap();
    assert_eq!(row["daemon_id"], win.as_str());
    let summary = listed["summary"].as_array().expect("summary");
    let theirs = summary
        .iter()
        .find(|s| s["daemon_id"] == win.as_str())
        .expect("the linked computer's summary");
    assert_eq!(theirs["running"], 1);
    assert!(theirs.get("stale").is_none(), "{theirs}");

    // The bot's computer announces a change; this one relays it as its own.
    let at = chrono::Utc::now().to_rfc3339();
    append(&path, &bash("w2", "dir", false, &at));
    let pushed = t
        .mac_client
        .wait_for(|f| f["type"] == "commands_changed" && f["daemon_id"] == win.as_str())
        .await;
    assert_eq!(pushed["bot_id"], t.linked_windev.as_str());
}
