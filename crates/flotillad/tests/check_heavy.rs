//! H-334: a computer runs one heavy check (a Rust build) at a time. A
//! second heavy check waits for the slot, an ordinary wait that never
//! counts as unroutable, while a light check still runs beside the first.
//! `heavy = false` in `checks.toml` lets a cargo check skip the slot.

use crate::common;

use std::collections::BTreeMap;
use std::time::Duration;

use common::prs::{branch, give, head, set_policy, setup_with, Repo};
use flotillad::kanban::model::Role;
use flotillad::prs::check_jobs::{dispatch, SYSTEM_RUNNER};
use flotillad::prs::check_model::{CheckResult, CheckRun};
use flotillad::releases::machines;
use serde_json::json;

/// Two heavy checks long enough to overlap, a cargo check that says it
/// isn't heavy, and a light one.
#[cfg(not(windows))]
const POLICY: &str = r#"
[[check]]
name = "rust-a"
run = "sleep 3"
needs = ["cargo"]

[[check]]
name = "rust-b"
run = "sleep 3"
needs = ["cargo"]

[[check]]
name = "fmt"
run = "true"
needs = ["cargo"]
heavy = false

[[check]]
name = "lint"
run = "true"
"#;

#[cfg(windows)]
const POLICY: &str = r#"
[[check]]
name = "rust-a"
run = "ping -n 4 127.0.0.1 >NUL"
needs = ["cargo"]

[[check]]
name = "rust-b"
run = "ping -n 4 127.0.0.1 >NUL"
needs = ["cargo"]

[[check]]
name = "fmt"
run = "exit 0"
needs = ["cargo"]
heavy = false

[[check]]
name = "lint"
run = "exit 0"
"#;

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

async fn opened(r: &mut Repo) -> String {
    give(r, 0, Role::Lead);
    set_policy(r, POLICY);
    let tools: BTreeMap<String, String> = [("cargo", "1.90.0"), ("os", "macos")]
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    let app = &r.pair.d.app;
    let at = app.db.board_read(machines::this_computer).unwrap();
    app.db
        .board_tx(|t| t.set_machine_tools(&at, &tools))
        .unwrap();
    let item = r.card("Rust", "doing");
    let tree = branch(r, "H-1-rust", "crates/rust.rs");
    r.bots[1]
        .call("pr_open", json!({"item": item, "branch": "H-1-rust"}))
        .await;
    head(&tree)
}

#[tokio::test]
async fn one_heavy_check_at_a_time_per_computer() {
    // Room for every check at once, so only the heavy slot holds one back.
    let mut r = setup_with(|cfg| cfg.checks.jobs_per_machine = 8).await;
    let sha = opened(&mut r).await;
    let app = r.pair.d.app.clone();
    dispatch(&app, &r.project).await.unwrap();

    let (a, b) = (check(&r, &sha, "rust-a"), check(&r, &sha, "rust-b"));
    let (running, waiting) = if a.runner.is_some() { (a, b) } else { (b, a) };
    assert_eq!(
        running.runner.as_deref(),
        Some(SYSTEM_RUNNER),
        "{running:?}"
    );
    assert_eq!(waiting.result, CheckResult::Queued, "{waiting:?}");
    assert!(waiting.runner.is_none());
    let note = waiting.note.clone().unwrap_or_default();
    assert!(note.contains("another heavy check"), "{note}");
    // Waiting for the slot is not unroutable: no Needs-you clock starts.
    assert_eq!(waiting.unroutable_since, None);
    assert_eq!(waiting.to_json()["unroutable"], false);
    // The light check and the cargo check that says it isn't heavy run
    // beside the heavy one.
    for light in ["fmt", "lint"] {
        let run = check(&r, &sha, light);
        assert_eq!(run.runner.as_deref(), Some(SYSTEM_RUNNER), "{run:?}");
    }

    // Routing again while the first still runs changes nothing.
    dispatch(&app, &r.project).await.unwrap();
    assert!(check(&r, &sha, &waiting.name).runner.is_none());

    // Once the first is done, the slot is free and the second runs.
    assert_eq!(
        finished(&r, &sha, &running.name).await.result,
        CheckResult::Pass
    );
    dispatch(&app, &r.project).await.unwrap();
    let next = check(&r, &sha, &waiting.name);
    assert_eq!(next.runner.as_deref(), Some(SYSTEM_RUNNER), "{next:?}");
    assert_eq!(
        finished(&r, &sha, &waiting.name).await.result,
        CheckResult::Pass
    );
}
