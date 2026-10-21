//! Each project's weekly token budget (0.24.0, `usage_v1`): its use over
//! the last 7 local days, and whether its on-demand bots are held. The
//! lead and always-on bots keep running whatever the budget says.

use std::collections::HashSet;

use chrono::{DateTime, Duration, Local, NaiveDate, TimeZone, Utc};
use serde_json::{json, Value};

use crate::app::AppState;
use crate::db::{StoredBudget, Tokens};

/// A project's budget as it stands.
#[derive(Debug, Clone, PartialEq)]
pub struct Budget {
    pub weekly_tokens: u64,
    /// Every kind of token over the week.
    pub used: u64,
    /// The week's start: local midnight six days ago.
    pub since: DateTime<Utc>,
    pub extended_until: Option<DateTime<Utc>>,
    /// Used up, and not extended: its on-demand bots don't wake.
    pub reached: bool,
}

impl Budget {
    /// Used, in percent of the budget, rounded down.
    pub fn percent(&self) -> u64 {
        if self.weekly_tokens == 0 {
            return 100;
        }
        u64::try_from(u128::from(self.used) * 100 / u128::from(self.weekly_tokens))
            .unwrap_or(u64::MAX)
    }

    pub fn over(&self) -> bool {
        self.used >= self.weekly_tokens
    }
}

/// Local midnight starting `day`.
pub fn midnight(day: NaiveDate) -> DateTime<Utc> {
    Local
        .from_local_datetime(&day.and_time(chrono::NaiveTime::MIN))
        .earliest()
        .map_or_else(
            || day.and_time(chrono::NaiveTime::MIN).and_utc(),
            |t| t.with_timezone(&Utc),
        )
}

pub fn today() -> NaiveDate {
    Local::now().date_naive()
}

/// The budget's week: today and the six days before.
pub fn week_start() -> NaiveDate {
    today() - Duration::days(6)
}

/// The next local midnight: when Extend today ends.
pub fn tomorrow() -> DateTime<Utc> {
    midnight(today() + Duration::days(1))
}

/// The project's tokens of every kind from `since` (a local date) on.
pub fn tokens_since(app: &AppState, project_id: &str, since: NaiveDate) -> anyhow::Result<Tokens> {
    let mut total = Tokens::default();
    for row in app
        .db
        .usage_rows(&since.to_string(), Some(project_id), None)?
    {
        total.add(&row.tokens);
    }
    Ok(total)
}

fn state(app: &AppState, stored: StoredBudget) -> anyhow::Result<Budget> {
    let start = week_start();
    let used = tokens_since(app, &stored.project_id, start)?.total();
    let extended = stored.extended_until.filter(|until| *until > Utc::now());
    Ok(Budget {
        weekly_tokens: stored.weekly_tokens,
        used,
        since: midnight(start),
        reached: used >= stored.weekly_tokens && extended.is_none(),
        extended_until: extended,
    })
}

/// The project's budget, if it has one.
pub fn of(app: &AppState, project_id: &str) -> anyhow::Result<Option<Budget>> {
    match app.db.project_budget(project_id)? {
        Some(stored) => Ok(Some(state(app, stored)?)),
        None => Ok(None),
    }
}

/// `ProjectBudget` as clients read it.
pub fn view(b: &Budget) -> Value {
    let mut v = json!({
        "weekly_tokens": b.weekly_tokens,
        "used": b.used,
        "since": b.since.with_timezone(&Local).to_rfc3339(),
        "reached": b.reached,
    });
    if let Some(until) = b.extended_until {
        v["extended_until"] = json!(until.with_timezone(&Local).to_rfc3339());
    }
    v
}

/// `project.budget`: the budget, or null.
pub fn project_json(app: &AppState, project_id: &str) -> Value {
    match of(app, project_id) {
        Ok(Some(b)) => view(&b),
        Ok(None) => Value::Null,
        Err(e) => {
            tracing::warn!(project_id, error = %e, "could not read the project's budget");
            Value::Null
        }
    }
}

/// The projects whose on-demand bots are held now.
pub fn held(app: &AppState) -> anyhow::Result<HashSet<String>> {
    let mut out = HashSet::new();
    for stored in app.db.project_budgets()? {
        let id = stored.project_id.clone();
        if state(app, stored)?.reached {
            out.insert(id);
        }
    }
    Ok(out)
}

/// Holds or releases the on-demand bots of every project by its budget.
pub fn refresh(app: &AppState) {
    match held(app) {
        Ok(held) => app.supervisor.set_budget_held(held),
        Err(e) => tracing::warn!(error = %e, "could not read the projects' budgets"),
    }
}
