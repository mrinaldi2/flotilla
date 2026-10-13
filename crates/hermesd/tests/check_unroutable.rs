//! H-318: checks see the derived tool PATH, and a check no reported
//! computer can run is unroutable: it says so, routes once a probe reports
//! the tool, and after an hour asks the owner and DevOps.

use crate::common;

use std::collections::BTreeMap;
use std::time::Duration;

use chrono::Utc;
use common::prs::{branch, give, head, set_policy, setup, Repo};
use common::WsClient;
use hermesd::kanban::model::Role;
use hermesd::prs::check_jobs::{dispatch, SYSTEM_RUNNER};
use hermesd::prs::check_model::{CheckResult, CheckRun};
use hermesd::releases::machines;
use serde_json::{json, Value};

/// The `path` check echoes its PATH in the computer's shell: `sh -c` here,
/// `cmd /C` on Windows, which prints `$PATH` as it is.
#[cfg(not(windows))]
const POLICY: &str = r#"
[[check]]
name = "path"
run = "echo \"PATH=$PATH\""
paths = ["crates/**"]

[[check]]
name = "desktop"
run = "true"
needs = ["pnpm"]
paths = ["crates/**"]
"#;

#[cfg(windows)]
const POLICY: &str = r#"
[[check]]
name = "path"
run = "echo \"PATH=%PATH%\""
paths = ["crates/**"]

[[check]]
name = "desktop"
run = "true"
needs = ["pnpm"]
paths = ["crates/**"]
"#;

const DEVOPS: usize = 2;

fn tools(r: &Repo, list: &[(&str, &str)]) {
    let tools: BTreeMap<String, String> = list
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    let app = &r.pair.d.app;
    let at = app.db.board_read(machines::this_computer).unwrap();
    app.db
        .board_tx(|t| t.set_machine_tools(&at, &tools))
        .unwrap();
}

async fn opened(r: &mut Repo) -> String {
    give(r, 0, Role::Lead);
    give(r, DEVOPS, Role::Devops);
    set_policy(r, POLICY);
    tools(r, &[("cargo", "1.90.0"), ("os", "macos")]);
    let item = r.card("Desktop", "doing");
    let tree = branch(r, "H-1-desktop", "crates/desktop.rs");
    r.bots[1]
        .call("pr_open", json!({"item": item, "branch": "H-1-desktop"}))
        .await;
    head(&tree)
}

fn check(r: &Repo, sha: &str, name: &str) -> CheckRun {
    r.pair
        .d
        .app
        .db
        .board_read(|t| t.check_run(&r.project, sha, name))
        .unwrap()
        .unwrap()
}

async fn finished(r: &Repo, sha: &str, name: &str) -> CheckRun {
    for _ in 0..1800 {
        let run = check(r, sha, name);
        if run.result.is_final() {
            return run;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("{name} never finished: {:?}", check(r, sha, name));
}

fn published(r: &Repo, run: &CheckRun) -> String {
    let app = &r.pair.d.app;
    let project = app.db.get_project(&r.project).unwrap().unwrap();
    let log = run.log_artifact.as_deref().expect("a log");
    std::fs::read_to_string(hermesd::paths::artifacts_dir(&app.cfg, &project.dir_name).join(log))
        .unwrap()
}

/// Moves every unroutable check's first time back by `hours`.
fn unroutable_hours_ago(r: &Repo, hours: i64) {
    let raw = rusqlite::Connection::open(r.pair.d.app.cfg.db_path()).unwrap();
    let at = (Utc::now() - chrono::Duration::hours(hours)).to_rfc3339();
    raw.execute(
        "UPDATE check_unroutable SET since = ?1",
        rusqlite::params![at],
    )
    .unwrap();
}

async fn rows(r: &Repo) -> Vec<Value> {
    let mut app = WsClient::connect(&r.pair.d).await;
    let out = app
        .request(json!({"type": "attention_rows", "project_id": r.project}))
        .await;
    out["attention_rows"]["rows"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|row| row["kind"] == "CHECK_UNROUTABLE")
        .collect()
}

/// The DevOps bot's daemon notes containing `needle`, joined; empty when
/// none came within a second.
async fn notes(r: &mut Repo, needle: &str) -> String {
    let mut seen = String::new();
    for _ in 0..20 {
        let inbox = r.bots[DEVOPS].call("check_inbox", json!({})).await;
        for m in inbox["messages"].as_array().into_iter().flatten() {
            if let Some(body) = m["body"].as_str().filter(|b| b.contains(needle)) {
                seen.push_str(body);
            }
        }
        if !seen.is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    seen
}

/// The runner's command sees the PATH the probe found its tools on.
#[tokio::test]
async fn a_check_runs_with_the_derived_tool_path() {
    let mut r = setup().await;
    let sha = opened(&mut r).await;
    dispatch(&r.pair.d.app.clone(), &r.project).await.unwrap();
    let run = finished(&r, &sha, "path").await;
    assert_eq!(run.result, CheckResult::Pass, "{run:?}");
    let want = hermesd::tool_path::tool_path();
    let log = published(&r, &run);
    assert!(
        log.contains(&format!("PATH={}", want.to_string_lossy())),
        "{log}"
    );
}

/// AC1: no computer reported pnpm: unroutable, not waiting; an hour on, a
/// Needs-you row and one note to DevOps; once a probe reports pnpm it
/// routes and the flag clears.
#[tokio::test]
async fn an_unroutable_check_says_so_asks_after_an_hour_and_routes_after_a_probe() {
    let mut r = setup().await;
    let sha = opened(&mut r).await;
    let app = r.pair.d.app.clone();
    dispatch(&app, &r.project).await.unwrap();

    let stuck = check(&r, &sha, "desktop");
    assert_eq!(stuck.result, CheckResult::Queued);
    assert!(stuck.runner.is_none());
    let since = stuck.unroutable_since.expect("unroutable");
    let note = stuck.note.clone().unwrap();
    assert!(
        note.starts_with("no computer can run this: needs pnpm; ") && note.contains("has cargo"),
        "{note}"
    );
    assert_eq!(stuck.to_json()["unroutable"], true);
    assert!(rows(&r).await.is_empty(), "not for an hour");

    // Routing again keeps the first time.
    dispatch(&app, &r.project).await.unwrap();
    assert_eq!(check(&r, &sha, "desktop").unroutable_since, Some(since));

    unroutable_hours_ago(&r, 2);
    dispatch(&app, &r.project).await.unwrap();
    let found = rows(&r).await;
    assert_eq!(found.len(), 1, "{found:?}");
    let title = found[0]["title"].as_str().unwrap();
    assert!(
        title.starts_with("No computer can run check desktop (PR #1): needs pnpm"),
        "{title}"
    );
    let told = notes(&mut r, "Check desktop on PR #1").await;
    assert!(told.contains("no computer can run this"), "{told}");
    dispatch(&app, &r.project).await.unwrap();
    assert!(notes(&mut r, "Check desktop").await.is_empty(), "told once");

    // A probe reports pnpm: the next pass routes it and the flag clears.
    tools(
        &r,
        &[("cargo", "1.90.0"), ("pnpm", "10.34.6"), ("os", "macos")],
    );
    dispatch(&app, &r.project).await.unwrap();
    let routed = check(&r, &sha, "desktop");
    assert_eq!(routed.runner.as_deref(), Some(SYSTEM_RUNNER));
    assert_eq!(routed.unroutable_since, None);
    assert!(rows(&r).await.is_empty());
    assert_eq!(
        finished(&r, &sha, "desktop").await.result,
        CheckResult::Pass
    );
}
