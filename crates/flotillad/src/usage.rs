//! Usage and budgets (0.24.0, `usage_v1`). Each bot's transcript is read as
//! its turns end (and every minute while it works) into a table per bot and
//! local day; see [`ingest`]. The counts feed `usage_get`, the projects
//! home's usage line, each project's weekly budget, and the bot's
//! `rate_limited` state when its runtime refuses for its rate limit.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use bus::{Bot, BotState};
use chrono::{DateTime, Local, Utc};

use crate::app::AppState;
use crate::events::{Internal, Push};

pub mod budget;
pub mod ingest;
pub mod parse;
pub mod prices;
pub mod report;
mod reset;

/// The most often `usage_updated` is pushed for one project.
const PUSH_EVERY: Duration = Duration::from_secs(60);
/// How often a working bot's transcript is read mid-turn.
const READ_WORKING_EVERY: Duration = Duration::from_secs(60);
/// The watcher's tick.
const TICK: Duration = Duration::from_secs(5);
/// A rate limit without a reset time counts only this soon after it came.
const UNTIMED_LIMIT_FRESH: chrono::Duration = chrono::Duration::minutes(15);
/// Counted entries older than this are forgotten: past every range.
const KEEP_SEEN_DAYS: i64 = 35;
/// The reason a rate limited bot shows when its runtime gave no reset time.
pub const LIMITED_UNTIMED: &str = "rate limited by the runtime";

/// The watcher's memory.
#[derive(Default)]
pub struct Usage {
    /// When each project's last `usage_updated` went out.
    pushed: Mutex<HashMap<String, Instant>>,
    /// Projects with a change not pushed yet.
    dirty: Mutex<HashSet<String>>,
    /// When each bot's transcript was last read.
    read: Mutex<HashMap<String, Instant>>,
    /// Bots set rate limited, and when that resets.
    limited: Mutex<HashMap<String, Option<DateTime<Utc>>>>,
    /// The day the counted entries were last pruned.
    pruned: Mutex<Option<String>>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Reads the bot's transcripts: its counts, its project's budget, and its
/// rate limit. Called as a turn ends.
pub fn record(app: &AppState, bot: &Bot) {
    lock(&app.usage.read).insert(bot.id.clone(), Instant::now());
    let read = match ingest::read(app, bot) {
        Ok(read) => read,
        Err(e) => {
            tracing::debug!(bot_id = %bot.id, error = %e, "usage read failed");
            return;
        }
    };
    if read.changed {
        changed(app, &bot.project_id);
        budget::refresh(app);
    }
    if let Some((at, resets_at)) = read.limited {
        rate_limited(app, &bot.id, at, resets_at);
    }
}

/// A project's counts changed: push now, or once a minute has passed.
pub fn changed(app: &AppState, project_id: &str) {
    let due = lock(&app.usage.pushed)
        .get(project_id)
        .is_none_or(|at| at.elapsed() >= PUSH_EVERY);
    if due {
        push(app, project_id);
    } else {
        lock(&app.usage.dirty).insert(project_id.to_string());
    }
}

fn push(app: &AppState, project_id: &str) {
    lock(&app.usage.pushed).insert(project_id.to_string(), Instant::now());
    lock(&app.usage.dirty).remove(project_id);
    app.events.push(Push::UsageUpdated {
        project_id: project_id.to_string(),
    });
}

/// The pushes held back whose minute has passed.
fn flush(app: &AppState) {
    let due: Vec<String> = {
        let pushed = lock(&app.usage.pushed);
        lock(&app.usage.dirty)
            .iter()
            .filter(|p| pushed.get(*p).is_none_or(|at| at.elapsed() >= PUSH_EVERY))
            .cloned()
            .collect()
    };
    for project_id in due {
        push(app, &project_id);
    }
}

/// The runtime refused the bot for its rate limit at `at`: it is
/// `rate_limited` until `resets_at`, which is its reason.
fn rate_limited(app: &AppState, bot_id: &str, at: DateTime<Utc>, resets_at: Option<DateTime<Utc>>) {
    let now = Utc::now();
    let current = match resets_at {
        Some(reset) => reset > now,
        None => now - at < UNTIMED_LIMIT_FRESH,
    };
    if !current {
        return;
    }
    let reason = resets_at.map_or_else(
        || LIMITED_UNTIMED.to_string(),
        |t| t.with_timezone(&Local).to_rfc3339(),
    );
    lock(&app.usage.limited).insert(bot_id.to_string(), resets_at);
    app.supervisor
        .set_state(bot_id, BotState::RateLimited, &reason);
}

/// Bots whose rate limit reset are ready again; ones that moved on are
/// forgotten.
fn clear_limits(app: &AppState) {
    let now = Utc::now();
    let bots: Vec<(String, Option<DateTime<Utc>>)> = lock(&app.usage.limited)
        .iter()
        .map(|(id, until)| (id.clone(), *until))
        .collect();
    for (bot_id, until) in bots {
        let still = app.supervisor.state(&bot_id).0 == BotState::RateLimited;
        if !still {
            lock(&app.usage.limited).remove(&bot_id);
        } else if until.is_some_and(|t| t <= now) {
            lock(&app.usage.limited).remove(&bot_id);
            app.supervisor
                .set_state(&bot_id, BotState::Ready, "rate limit reset");
        }
    }
}

/// Reads every working bot whose transcript wasn't read for a minute.
fn read_working(app: &AppState, all: bool) {
    let Ok(bots) = app.db.list_bots(None) else {
        return;
    };
    for bot in bots.iter().filter(|b| !b.is_linked()) {
        let working = matches!(
            app.supervisor.state(&bot.id).0,
            BotState::Working | BotState::WaitingForApproval
        );
        let stale = lock(&app.usage.read)
            .get(&bot.id)
            .is_none_or(|at| at.elapsed() >= READ_WORKING_EVERY);
        if all || (working && stale) {
            record(app, bot);
        }
    }
}

fn prune(app: &AppState) {
    let today = budget::today().to_string();
    if lock(&app.usage.pruned).as_deref() == Some(today.as_str()) {
        return;
    }
    let before = budget::today() - chrono::Duration::days(KEEP_SEEN_DAYS);
    if let Err(e) = app.db.prune_usage_seen(&before.to_string()) {
        tracing::warn!(error = %e, "could not prune counted usage");
    }
    *lock(&app.usage.pruned) = Some(today);
}

/// Reads each bot's transcript as its turn ends, and keeps the pushes, the
/// budgets and the rate limits current.
pub async fn watch(app: Arc<AppState>) {
    use tokio::sync::broadcast::error::RecvError;
    let mut internal = app.events.subscribe_internal();
    let mut tick = tokio::time::interval(TICK);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut first = true;
    loop {
        tokio::select! {
            event = internal.recv() => match event {
                Ok(Internal::BotDone { bot_id, .. }) => {
                    tokio::spawn(after_turn(app.clone(), bot_id));
                }
                Ok(_) => {}
                Err(RecvError::Lagged(_)) => {}
                Err(RecvError::Closed) => break,
            },
            _ = tick.tick() => {
                let (app, all) = (app.clone(), std::mem::take(&mut first));
                let _ = tokio::task::spawn_blocking(move || {
                    read_working(&app, all);
                    flush(&app);
                    clear_limits(&app);
                    budget::refresh(&app);
                    prune(&app);
                })
                .await;
            }
        }
    }
}

/// A turn ended: read it now, and again shortly in case its last lines
/// were still being written.
async fn after_turn(app: Arc<AppState>, bot_id: String) {
    for wait in [Duration::ZERO, Duration::from_secs(3)] {
        tokio::time::sleep(wait).await;
        let (app, id) = (app.clone(), bot_id.clone());
        let _ = tokio::task::spawn_blocking(move || {
            if let Ok(Some(bot)) = app.db.get_live_bot(&id) {
                record(&app, &bot);
            }
        })
        .await;
    }
}
