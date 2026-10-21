//! Usage (0.24.0, `usage_v1`): a bot's transcripts read into a table per
//! bot and day, once however often they are read; `usage_get` over a range
//! with each project's and bot's share; the price table and its estimates.

use std::io::Write;
use std::path::PathBuf;

use chrono::{Duration, Local, Utc};
use flotillad::db::{Tokens, UsageEntry};
use serde_json::{json, Value};

use crate::common::tasks::project_with_bots;
use crate::common::transcripts::transcript_dir;
use crate::common::{TestDaemon, WsClient};

fn reply(id: &str, model: &str, input: u64, output: u64, read: u64, write: u64) -> Value {
    json!({
        "type": "assistant", "timestamp": Utc::now().to_rfc3339(), "uuid": format!("u-{id}-{output}"),
        "message": { "id": id, "model": model, "content": [{"type": "text", "text": "ok"}],
            "usage": { "input_tokens": input, "output_tokens": output,
                       "cache_read_input_tokens": read, "cache_creation_input_tokens": write } }
    })
}

fn prompt(uuid: &str) -> Value {
    json!({"type": "user", "uuid": uuid, "timestamp": Utc::now().to_rfc3339(),
           "message": {"content": "Do the next step"}})
}

fn append(path: &PathBuf, lines: &[Value]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .unwrap();
    for line in lines {
        writeln!(file, "{line}").unwrap();
    }
}

fn today(d: &TestDaemon, bot_id: &str) -> (Tokens, u64) {
    let day = Local::now().date_naive().to_string();
    let mut tokens = Tokens::default();
    let mut turns = 0;
    for row in d.app.db.usage_rows(&day, None, Some(bot_id)).unwrap() {
        tokens.add(&row.tokens);
        turns += row.turns;
    }
    (tokens, turns)
}

#[tokio::test]
async fn a_transcript_counts_once_however_often_it_is_read() {
    let (p, _) = project_with_bots(&["ann"]).await;
    let app = &p.d.app;
    let bot = app.db.get_bot(&p.ids[0]).unwrap().unwrap();
    let dir = transcript_dir(&app.cfg.user_home, &bot.workspace_path);
    let session = dir.join("session-1.jsonl");
    // Claude Code writes a reply's usage on each of its lines.
    append(
        &session,
        &[
            prompt("p1"),
            reply("msg_1", "claude-opus-5-5", 10, 5, 100, 20),
            reply("msg_1", "claude-opus-5-5", 10, 5, 100, 20),
            json!({"type": "user", "uuid": "r1", "message": {"content": [
                {"type": "tool_result", "tool_use_id": "t", "content": "done"}]}}),
            reply("msg_2", "claude-opus-5-5", 1, 7, 200, 0),
        ],
    );
    // A subagent's session counts to the same bot.
    append(
        &dir.join("session-1")
            .join("subagents")
            .join("agent-a.jsonl"),
        &[reply("msg_s", "claude-haiku-5-5", 3, 3, 0, 0)],
    );

    // The daemon's own watcher may read it too: either way, once.
    flotillad::usage::ingest::read(app, &bot).unwrap();
    let want = Tokens {
        input: 14,
        output: 15,
        cache_read: 300,
        cache_write: 20,
    };
    assert_eq!(today(&p.d, &bot.id), (want, 1));

    // Read again: nothing new.
    let again = flotillad::usage::ingest::read(app, &bot).unwrap();
    assert!(!again.changed);
    assert_eq!(today(&p.d, &bot.id), (want, 1));

    // A reply's later line with more output adds only what grew.
    append(&session, &[reply("msg_2", "claude-opus-5-5", 1, 9, 200, 0)]);
    flotillad::usage::ingest::read(app, &bot).unwrap();
    let grown = Tokens { output: 17, ..want };
    assert_eq!(today(&p.d, &bot.id), (grown, 1));

    // A rewritten file is read from its start, and still counts once.
    let whole = std::fs::read_to_string(&session).unwrap();
    std::fs::write(
        &session,
        whole.lines().take(3).collect::<Vec<_>>().join("\n") + "\n",
    )
    .unwrap();
    flotillad::usage::ingest::read(app, &bot).unwrap();
    assert_eq!(today(&p.d, &bot.id), (grown, 1));

    // The next prompt's turn counts on its reply, by model.
    append(
        &session,
        &[
            prompt("p2"),
            reply("msg_3", "claude-sonnet-5-5", 1, 1, 0, 0),
        ],
    );
    flotillad::usage::ingest::read(app, &bot).unwrap();
    let (_, turns) = today(&p.d, &bot.id);
    assert_eq!(turns, 2);
    let day = Local::now().date_naive().to_string();
    let rows = app.db.usage_rows(&day, None, Some(&bot.id)).unwrap();
    let models: Vec<&str> = rows.iter().map(|r| r.model.as_str()).collect();
    assert_eq!(
        models,
        ["claude-haiku-5-5", "claude-opus-5-5", "claude-sonnet-5-5"]
    );
    assert!(rows.iter().all(|r| r.project_id == bot.project_id));
}

/// Adds `tokens` of output to the bot on the day `days_ago`.
fn used(d: &TestDaemon, bot_id: &str, days_ago: i64, model: &str, output: u64) {
    let bot = d.app.db.get_bot(bot_id).unwrap().unwrap();
    let day = (Local::now().date_naive() - Duration::days(days_ago)).to_string();
    let key = format!("{bot_id}:{days_ago}:{model}:{output}");
    let entry = UsageEntry::Reply {
        key: key.clone(),
        day,
        model: model.to_string(),
        tokens: Tokens {
            output,
            ..Tokens::default()
        },
    };
    d.app
        .db
        .usage_apply(bot_id, &bot.project_id, &key, 0, &[entry])
        .unwrap();
}

async fn usage(c: &mut WsClient, req: Value) -> Value {
    let reply = c.request(req).await;
    assert_eq!(reply["type"], "usage", "{reply}");
    reply
}

#[tokio::test]
async fn usage_get_sums_a_range_with_each_share() {
    let (p, _) = crate::common::tasks::two_projects_one_bot_each().await;
    let (ann, bo) = (p.ids[0].clone(), p.ids[1].clone());
    used(&p.d, &ann, 0, "claude-opus-5-5", 3_000_000);
    used(&p.d, &bo, 0, "claude-sonnet-5-5", 1_000_000);
    used(&p.d, &ann, 3, "claude-opus-5-5", 4_000_000);
    used(&p.d, &bo, 20, "claude-sonnet-5-5", 2_000_000);
    used(&p.d, &bo, 40, "claude-sonnet-5-5", 9_000_000);
    let mut c = WsClient::connect(&p.d).await;

    let today = usage(&mut c, json!({"type": "usage_get", "range": "today"})).await;
    assert_eq!(today["total"]["tokens"], 4_000_000);
    assert_eq!(today["total"]["output"], 4_000_000);
    // Seeded estimates: Opus output at $20, Sonnet at $10 per million.
    assert_eq!(today["total"]["est_cost_usd"], 70.0);
    assert_eq!(today["days"].as_array().unwrap().len(), 1);
    let ann_project = p.d.app.db.get_bot(&ann).unwrap().unwrap().project_id;
    let first = &today["projects"][0];
    assert_eq!(first["project_id"], ann_project.as_str());
    assert_eq!(first["share"], 0.75);
    assert_eq!(first["name"], "alpha");
    assert_eq!(today["bots"][0]["bot_id"], ann.as_str());
    assert_eq!(today["bots"][0]["name"], "ann");
    assert_eq!(today["bots"][1]["share"], 0.25);
    assert_eq!(
        today["bots"][0]["daemon_id"],
        p.d.app.db.daemon_id().unwrap()
    );
    assert!(today["prices_as_of"]
        .as_str()
        .is_some_and(|s| !s.is_empty()));

    let week = usage(&mut c, json!({"type": "usage_get", "range": "7d"})).await;
    assert_eq!(week["total"]["tokens"], 8_000_000);
    let days = week["days"].as_array().unwrap();
    assert_eq!(days.len(), 7);
    assert_eq!(days[3]["totals"]["tokens"], 4_000_000, "oldest first");
    assert_eq!(days[6]["totals"]["tokens"], 4_000_000);
    assert_eq!(days[0]["totals"]["tokens"], 0);

    let month = usage(&mut c, json!({"type": "usage_get", "range": "30d"})).await;
    assert_eq!(
        month["total"]["tokens"], 10_000_000,
        "not the day 40 days ago"
    );

    let one = usage(
        &mut c,
        json!({"type": "usage_get", "range": "30d", "bot_id": bo}),
    )
    .await;
    assert_eq!(one["total"]["tokens"], 3_000_000);
    assert_eq!(one["bots"].as_array().unwrap().len(), 1);
    assert_eq!(one["projects"][0]["share"], 1.0);

    let bad = c
        .request(json!({"type": "usage_get", "range": "year"}))
        .await;
    assert_eq!(bad["code"], "invalid_request", "{bad}");
}

#[tokio::test]
async fn the_owner_sets_the_prices_the_estimates_use() {
    let (p, _) = project_with_bots(&["ann"]).await;
    used(&p.d, &p.ids[0], 0, "claude-opus-5-5", 1_000_000);
    let mut c = WsClient::connect(&p.d).await;
    let seeded = c.request(json!({"type": "usage_prices"})).await;
    assert_eq!(seeded["type"], "usage_prices", "{seeded}");
    let prices = seeded["prices"].as_array().unwrap();
    for family in ["fable", "opus", "sonnet", "haiku", "*"] {
        let row = prices
            .iter()
            .find(|p| p["model"] == family)
            .unwrap_or_else(|| panic!("no {family}: {seeded}"));
        assert_eq!(row["estimate"], true);
    }

    let table = json!([
        {"model": "opus", "input": 1, "output": 2, "cache_read": 0.1, "cache_write": 1.25},
    ]);
    // Not from the owner token a bot can read.
    let mut token = WsClient::connect_owner_token(&p.d).await;
    let refused = token
        .request(json!({"type": "set_usage_prices", "prices": table}))
        .await;
    assert_eq!(refused["code"], "forbidden", "{refused}");

    let set = c
        .request(json!({"type": "set_usage_prices", "prices": table}))
        .await;
    assert_eq!(set["type"], "usage_prices", "{set}");
    assert_eq!(set["prices"].as_array().unwrap().len(), 1);
    assert_eq!(set["prices"][0]["estimate"], false);
    let today = usage(&mut c, json!({"type": "usage_get", "range": "today"})).await;
    assert_eq!(today["total"]["est_cost_usd"], 2.0);

    let bad = c
        .request(json!({"type": "set_usage_prices", "prices": [{"model": "opus", "input": -1}]}))
        .await;
    assert_eq!(bad["code"], "invalid_request", "{bad}");
}
