//! The Commands page's list: its order, filters, pages and summary.

use serde_json::{json, Value};

use super::{answer, Filter, Part};

fn row(id: &str, status: &str, started_at: &str, command: &str) -> Value {
    json!({ "id": id, "bot_id": "b1", "status": status, "started_at": started_at,
            "command": command, "description": format!("{command} please") })
}

fn parts() -> Vec<Part> {
    let today = chrono::Utc::now().to_rfc3339();
    vec![
        Part {
            daemon_id: "mac".into(),
            computer_name: "Mac".into(),
            rows: vec![
                row("a", "done", "2026-10-01T10:00:00Z", "cargo test"),
                row("b", "running", "2026-10-01T09:00:00Z", "./serve.sh"),
                row("c", "failed", &today, "pnpm build"),
                row("d", "done", "2026-10-02T10:00:00Z", "git status"),
            ],
            stale: false,
        },
        Part {
            daemon_id: "pc".into(),
            computer_name: "PC".into(),
            rows: Vec::new(),
            stale: true,
        },
    ]
}

fn ids(v: &Value) -> Vec<&str> {
    v["commands"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap())
        .collect()
}

#[test]
fn running_first_then_newest_with_a_summary_per_computer() {
    let out = answer(parts(), &Filter::default());
    assert_eq!(out["type"], "commands");
    assert_eq!(ids(&out)[0], "b");
    assert_eq!(&ids(&out)[1..], ["c", "d", "a"]);
    assert_eq!(out["summary"][0]["running"], 1);
    assert_eq!(out["summary"][0]["failed_today"], 1);
    assert!(out["summary"][0].get("stale").is_none());
    assert_eq!(out["summary"][1]["stale"], true);
    assert!(out.get("next_before").is_none());
}

#[test]
fn status_and_query_filter_the_rows_but_not_the_summary() {
    let running = Filter {
        status: Some("running".into()),
        ..Filter::default()
    };
    assert_eq!(ids(&answer(parts(), &running)), ["b"]);
    let failed = Filter {
        status: Some("failed".into()),
        ..Filter::default()
    };
    let out = answer(parts(), &failed);
    assert_eq!(ids(&out), ["c"]);
    assert_eq!(out["summary"][0]["running"], 1);
    let query = Filter {
        query: Some("CARGO".into()),
        ..Filter::default()
    };
    assert_eq!(ids(&answer(parts(), &query)), ["a"]);
    let description = Filter {
        query: Some("status please".into()),
        ..Filter::default()
    };
    assert_eq!(ids(&answer(parts(), &description)), ["d"]);
}

#[test]
fn pages_follow_next_before() {
    let first = Filter {
        limit: Some(2),
        ..Filter::default()
    };
    let page = answer(parts(), &first);
    assert_eq!(ids(&page), ["b", "c"]);
    let next = Filter {
        limit: Some(2),
        before: page["next_before"].as_str().map(str::to_string),
        ..Filter::default()
    };
    let page = answer(parts(), &next);
    assert_eq!(ids(&page), ["d", "a"]);
    assert!(page.get("next_before").is_none());
}
