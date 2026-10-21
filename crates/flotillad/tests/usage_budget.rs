//! Budgets (0.24.0, `usage_v1`): a warning at 80% of a project's weekly
//! tokens; at 100% its on-demand bots' wakes are held while the lead keeps
//! running, until the owner raises the budget or extends today. And a bot
//! its runtime rate limits is `rate_limited` until the reset it gives.

use std::io::Write;

use bus::contract::home::AttentionKind;
use chrono::{Duration, Local, Utc};
use flotillad::db::{Tokens, UsageEntry};
use serde_json::{json, Value};

use crate::common::on_demand::{dm, queue, state, team, until_state, until_team};
use crate::common::transcripts::transcript_dir;
use crate::common::{frame_wait, TestDaemon, WsClient};

/// Adds `output` tokens of Opus to the bot `days_ago`.
fn used(d: &TestDaemon, bot_id: &str, days_ago: i64, output: u64) {
    let bot = d.app.db.get_bot(bot_id).unwrap().unwrap();
    let day = (Local::now().date_naive() - Duration::days(days_ago)).to_string();
    let key = format!("{bot_id}:{days_ago}:{output}");
    let entry = UsageEntry::Reply {
        key: key.clone(),
        day,
        model: "claude-opus-5-5".into(),
        tokens: Tokens {
            output,
            ..Tokens::default()
        },
    };
    d.app
        .db
        .usage_apply(bot_id, &bot.project_id, &key, 0, &[entry])
        .unwrap();
    flotillad::usage::budget::refresh(&d.app);
}

/// The project's Needs you rows of the budget's kinds: kind, title, weight.
fn budget_rows(d: &TestDaemon, project: &str) -> Vec<(AttentionKind, String, u32)> {
    flotillad::overview::attention_rows(&d.app, project)
        .unwrap()
        .rows
        .into_iter()
        .filter(|r| {
            matches!(
                r.kind(),
                AttentionKind::BudgetWarning | AttentionKind::BudgetReached
            )
        })
        .map(|r| (r.kind(), r.title.clone(), r.weight))
        .collect()
}

async fn budget(c: &mut WsClient, req: Value) -> Value {
    let reply = c.request(req).await;
    assert_eq!(reply["type"], "project", "{reply}");
    reply["project"]["budget"].clone()
}

#[tokio::test]
async fn a_used_budget_holds_on_demand_bots_until_extended() {
    let (p, mut bots, project) = team(1, &["lead", "a"]).await;
    let (lead, a) = (p.ids[0].clone(), p.ids[1].clone());
    let mut c = WsClient::connect(&p.d).await;
    until_state(&p.d, &a, &["asleep"]).await;

    // Only the owner's app or device sets it.
    let mut token = WsClient::connect_owner_token(&p.d).await;
    let refused = token
        .request(
            json!({"type": "set_project_budget", "project_id": project, "weekly_tokens": 1000}),
        )
        .await;
    assert_eq!(refused["code"], "forbidden", "{refused}");
    let set = budget(
        &mut c,
        json!({"type": "set_project_budget", "project_id": project, "weekly_tokens": 1000}),
    )
    .await;
    assert_eq!(set["weekly_tokens"], 1000);
    assert_eq!(set["used"], 0);
    assert_eq!(set["reached"], false);
    assert!(set["since"].is_string());
    assert!(budget_rows(&p.d, &project).is_empty());

    // 80%: a warning, nothing held.
    used(&p.d, &a, 0, 850);
    assert_eq!(
        budget_rows(&p.d, &project),
        [(
            AttentionKind::BudgetWarning,
            "p used 85% of its weekly budget".to_string(),
            1
        )]
    );

    // 100%, counting yesterday's: the on-demand bot's wake is held.
    used(&p.d, &a, 1, 200);
    let rows = budget_rows(&p.d, &project);
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(rows[0].0, AttentionKind::BudgetReached);
    assert_eq!(rows[0].2, 2);
    dm(&mut bots[0], "a", "note", "Have a look at this").await;
    let t = until_team(&mut c, |t| queue(t) == [a.clone()]).await;
    assert_eq!(t["queue"][0]["held"], "budget", "{t}");
    assert_eq!(state(&p.d, &a), "queued");
    let reason = p.d.app.supervisor.state(&a).1;
    assert!(reason.contains("budget"), "{reason}");

    // The lead keeps running, and its wakes aren't held.
    assert!(
        ["ready", "working", "starting"].contains(&state(&p.d, &lead).as_str()),
        "{}",
        state(&p.d, &lead)
    );
    let ok = c.request(json!({"type": "wake_bot", "bot_id": lead})).await;
    assert_eq!(ok["type"], "ok", "{ok}");
    let t = until_team(&mut c, |t| queue(t) == [a.clone()]).await;
    assert_eq!(t["queue"].as_array().unwrap().len(), 1, "{t}");

    // Extend today releases it until midnight.
    let extended = budget(
        &mut c,
        json!({"type": "budget_extend", "project_id": project}),
    )
    .await;
    assert_eq!(extended["reached"], false, "{extended}");
    assert!(extended["extended_until"].is_string(), "{extended}");
    until_team(&mut c, |t| queue(t).is_empty()).await;
    until_state(&p.d, &a, &["starting", "ready", "working"]).await;
    let rows = budget_rows(&p.d, &project);
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].0,
        AttentionKind::BudgetWarning,
        "extended: {rows:?}"
    );
}

#[tokio::test]
async fn raising_the_budget_releases_held_wakes() {
    let (p, mut bots, project) = team(1, &["lead", "a"]).await;
    let a = p.ids[1].clone();
    let mut c = WsClient::connect(&p.d).await;
    until_state(&p.d, &a, &["asleep"]).await;
    budget(
        &mut c,
        json!({"type": "set_project_budget", "project_id": project, "weekly_tokens": 100}),
    )
    .await;
    used(&p.d, &a, 0, 150);
    dm(&mut bots[0], "a", "note", "Have a look at this").await;
    let t = until_team(&mut c, |t| queue(t) == [a.clone()]).await;
    assert_eq!(t["queue"][0]["held"], "budget", "{t}");

    let raised = budget(
        &mut c,
        json!({"type": "set_project_budget", "project_id": project, "weekly_tokens": 10_000}),
    )
    .await;
    assert_eq!(raised["used"], 150);
    assert_eq!(raised["reached"], false);
    until_team(&mut c, |t| queue(t).is_empty()).await;
    until_state(&p.d, &a, &["starting", "ready", "working"]).await;
    assert!(budget_rows(&p.d, &project).is_empty());

    // No budget at all: null, and nothing to extend.
    let none = budget(
        &mut c,
        json!({"type": "set_project_budget", "project_id": project, "weekly_tokens": null}),
    )
    .await;
    assert!(none.is_null());
    let conflict = c
        .request(json!({"type": "budget_extend", "project_id": project}))
        .await;
    assert_eq!(conflict["code"], "conflict", "{conflict}");
}

#[tokio::test]
async fn a_rate_limited_bot_waits_for_its_reset() {
    let (p, _, _) = team(600, &["ann"]).await;
    let ann = p.ids[0].clone();
    until_state(&p.d, &ann, &["ready"]).await;
    let bot = p.d.app.db.get_bot(&ann).unwrap().unwrap();
    let reset = Utc::now() + Duration::seconds(3);
    let line = json!({
        "type": "assistant", "timestamp": Utc::now().to_rfc3339(), "uuid": "rl",
        "isApiErrorMessage": true, "error": "rate_limit",
        "message": { "id": "x", "model": "<synthetic>",
            "usage": { "input_tokens": 0, "output_tokens": 0 },
            "content": [{"type": "text",
                         "text": format!("Claude AI usage limit reached|{}", reset.timestamp())}] }
    });
    let dir = transcript_dir(&p.d.app.cfg.user_home, &bot.workspace_path);
    std::fs::create_dir_all(&dir).unwrap();
    let mut file = std::fs::File::create(dir.join("s.jsonl")).unwrap();
    writeln!(file, "{line}").unwrap();

    flotillad::usage::record(&p.d.app, &bot);
    let (now, reason) = p.d.app.supervisor.state(&ann);
    assert_eq!(now.as_str(), "rate_limited");
    let until = chrono::DateTime::parse_from_rfc3339(&reason).expect("the reset is the reason");
    assert_eq!(until.timestamp(), reset.timestamp());

    // Past the reset it is ready again.
    let deadline = tokio::time::Instant::now() + frame_wait() + std::time::Duration::from_secs(5);
    while state(&p.d, &ann) == "rate_limited" {
        assert!(tokio::time::Instant::now() < deadline, "still rate limited");
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert_eq!(state(&p.d, &ann), "ready");
}

fn number(v: &Value) -> u64 {
    v.as_u64()
        .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
        .unwrap_or(0)
}

#[tokio::test]
async fn the_projects_home_shows_usage_today() {
    let (p, _, project) = team(600, &["lead"]).await;
    let mut c = WsClient::connect(&p.d).await;
    budget(
        &mut c,
        json!({"type": "set_project_budget", "project_id": project, "weekly_tokens": 1000}),
    )
    .await;
    used(&p.d, &p.ids[0], 0, 500);
    used(&p.d, &p.ids[0], 2, 100);
    let reply = c.request(json!({"type": "projects_overview"})).await;
    assert_eq!(reply["type"], "projects_overview", "{reply}");
    let row = reply["overview"]["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["project_id"] == project.as_str())
        .unwrap_or_else(|| panic!("no row: {reply}"))
        .clone();
    let usage = &row["usage"];
    assert_eq!(number(&usage["tokens_today"]), 500, "{row}");
    assert_eq!(number(&usage["tokens_week"]), 600);
    assert_eq!(number(&usage["budget_week"]), 1000);
    // 500 Opus output tokens at the seeded $20 per million.
    assert!((usage["est_cost_today_usd"].as_f64().unwrap() - 0.01).abs() < 1e-9);
    assert_ne!(usage["budget_reached"], true);
}
