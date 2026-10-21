//! `usage_get` (0.24.0, `usage_v1`): the totals over a range, each
//! project's and bot's share of them, the days, and an estimated cost.

use std::collections::BTreeMap;

use chrono::{Duration, Local};
use serde_json::{json, Value};

use super::budget;
use super::prices::cost;
use crate::app::AppState;
use crate::db::{Db, Tokens, UsagePrice, UsageRow};

/// `today`, `7d` or `30d`: how many local days back from today.
pub fn range_days(range: &str) -> Option<i64> {
    match range {
        "today" => Some(1),
        "7d" => Some(7),
        "30d" => Some(30),
        _ => None,
    }
}

/// Tokens, turns and their estimated cost, summed.
#[derive(Debug, Default, Clone)]
pub struct Totals {
    pub tokens: Tokens,
    pub turns: u64,
    pub usd: f64,
}

impl Totals {
    pub fn add(&mut self, row: &UsageRow, prices: &[UsagePrice]) {
        self.tokens.add(&row.tokens);
        self.turns += row.turns;
        self.usd += cost(prices, &row.model, &row.tokens);
    }

    pub fn view(&self) -> Value {
        json!({
            "input": self.tokens.input,
            "output": self.tokens.output,
            "cache_read": self.tokens.cache_read,
            "cache_write": self.tokens.cache_write,
            "tokens": self.tokens.total(),
            "turns": self.turns,
            "est_cost_usd": (self.usd * 10_000.0).round() / 10_000.0,
        })
    }
}

fn share(part: &Totals, of: &Totals) -> f64 {
    let total = of.tokens.total();
    if total == 0 {
        return 0.0;
    }
    part.tokens.total() as f64 / total as f64
}

/// The `usage` reply, without its `req_id`.
pub fn usage(
    app: &AppState,
    range: &str,
    project_id: Option<&str>,
    bot_id: Option<&str>,
) -> anyhow::Result<Value> {
    let Some(days) = range_days(range) else {
        anyhow::bail!("range must be today, 7d or 30d");
    };
    let today = budget::today();
    let start = today - Duration::days(days - 1);
    let rows = app.db.usage_rows(&start.to_string(), project_id, bot_id)?;
    let prices = app.db.usage_prices()?;
    let mut total = Totals::default();
    let mut projects: BTreeMap<String, Totals> = BTreeMap::new();
    let mut bots: BTreeMap<String, (String, Totals)> = BTreeMap::new();
    let mut by_day: BTreeMap<String, Totals> = BTreeMap::new();
    for row in &rows {
        total.add(row, &prices);
        projects
            .entry(row.project_id.clone())
            .or_default()
            .add(row, &prices);
        bots.entry(row.bot_id.clone())
            .or_insert_with(|| (row.project_id.clone(), Totals::default()))
            .1
            .add(row, &prices);
        by_day.entry(row.day.clone()).or_default().add(row, &prices);
    }
    let daemon_id = app.db.daemon_id()?;
    let mut project_list: Vec<Value> = projects
        .iter()
        .map(|(id, totals)| {
            let project = app.db.get_project(id).ok().flatten();
            let mut v = json!({
                "project_id": id,
                "name": project.as_ref().map(Db::display_project_name).unwrap_or_default(),
                "totals": totals.view(),
                "share": share(totals, &total),
            });
            if let Ok(Some(b)) = budget::of(app, id) {
                v["budget"] = budget::view(&b);
            }
            v
        })
        .collect();
    sort_by_tokens(&mut project_list);
    let mut bot_list: Vec<Value> = bots
        .iter()
        .map(|(id, (project_id, totals))| {
            let bot = app.db.get_bot(id).ok().flatten();
            json!({
                "bot_id": id,
                "name": bot.as_ref().map(Db::display_name).unwrap_or_default(),
                "project_id": project_id,
                "daemon_id": daemon_id,
                "totals": totals.view(),
                "share": share(totals, &total),
            })
        })
        .collect();
    sort_by_tokens(&mut bot_list);
    let day_list: Vec<Value> = (0..days)
        .map(|n| {
            let date = (start + Duration::days(n)).to_string();
            let totals = by_day.get(&date).cloned().unwrap_or_default();
            json!({ "date": date, "totals": totals.view() })
        })
        .collect();
    let as_of = app
        .db
        .usage_prices_as_of()?
        .map(|t| t.with_timezone(&Local).to_rfc3339())
        .unwrap_or_default();
    Ok(json!({
        "type": "usage",
        "range": range,
        "total": total.view(),
        "projects": project_list,
        "bots": bot_list,
        "days": day_list,
        "prices_as_of": as_of,
    }))
}

/// Most tokens first.
fn sort_by_tokens(list: &mut [Value]) {
    list.sort_by_key(|v| std::cmp::Reverse(v["totals"]["tokens"].as_u64().unwrap_or(0)));
}

/// `ProjectRow.usage`: today's tokens and estimate, the week's use and the
/// budget, for the "usage today" line on the project's card.
pub fn project_usage(
    app: &AppState,
    project_id: &str,
) -> anyhow::Result<bus::contract::home::ProjectUsage> {
    let prices = app.db.usage_prices()?;
    let today = budget::today().to_string();
    let mut today_totals = Totals::default();
    let mut week = 0;
    for row in app
        .db
        .usage_rows(&budget::week_start().to_string(), Some(project_id), None)?
    {
        week += row.tokens.total();
        if row.day == today {
            today_totals.add(&row, &prices);
        }
    }
    let budget = budget::of(app, project_id)?;
    Ok(bus::contract::home::ProjectUsage {
        tokens_today: today_totals.tokens.total(),
        tokens_week: week,
        budget_week: budget.as_ref().map_or(0, |b| b.weekly_tokens),
        est_cost_today_usd: today_totals.usd,
        budget_reached: budget.is_some_and(|b| b.reached),
    })
}
