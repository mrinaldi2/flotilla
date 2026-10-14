//! The board keeper (0.19.0): a deterministic loop, no model and no tokens,
//! that keeps each board in step with what its bots actually do. It runs
//! every few minutes (`[keeper] tick_seconds`) and after each turn of a bot,
//! only on a board's home computer, never on a linked computer that mirrors
//! it. Per live card:
//!
//! 1. quiet in its work column (Doing), assignee idle: the assignee is asked
//!    once; still quiet later, the card is stale and the owner has a row;
//! 2. assigned in an open column (To do), not touched: the assignment is
//!    told again, once;
//! 3. changed in a turn of its assignee and not moved: the bot is asked
//!    once to move it, or to say why not;
//! 4. in a review column: a row, "waits for your review", while it is there;
//! 5. a refused move that would still be refused: a row naming the rule;
//! 6. past the board's `stale_after_hours`: its `stale` flag, pushed.
//!
//! A release-gated board gets 1's row (no note), 5 and 6 only: its guards
//! and flow are unchanged, and the keeper never moves a card on any board.
//! What the keeper did is kept in `keeper_mark`, so each condition acts once
//! until the card moves or changes hands (or, for 1, anything happens on
//! it), and a restart sends nothing again. Timers count from the daemon's
//! start at the earliest: after a long downtime nothing fires at once.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use tokio::sync::broadcast::error::RecvError;

use crate::app::AppState;
use crate::events::Push;

mod announce;
mod apply;
pub mod config;
mod gather;
mod memory;
mod rows;
pub mod rules;
mod text;

pub use config::KeeperConfig;
pub use memory::Keeper;
pub use rows::{waiting, Waiting};

use apply::{apply, Budget};
use rules::{Clock, Rule};

/// The most cards one turn's end asks about (rule 3).
const ASKS_PER_TURN: usize = 3;

/// Watch bots' turns, and pass over every board every `tick_seconds`.
pub fn spawn(app: Arc<AppState>) {
    let watched = app.clone();
    tokio::spawn(async move {
        let mut pushes = watched.events.subscribe_push();
        loop {
            match pushes.recv().await {
                Ok(Push::BotState {
                    bot_id,
                    state,
                    at,
                    item_id,
                    ..
                }) => {
                    let at = DateTime::parse_from_rfc3339(&at)
                        .map_or_else(|_| Utc::now(), |t| t.with_timezone(&Utc));
                    if let Some(started) = watched.keeper.on_state(&bot_id, state, at) {
                        let turn = Turn {
                            bot_id: &bot_id,
                            started,
                            card: item_id.as_deref(),
                        };
                        after_turn(&watched, &watched.keeper, &turn, Utc::now());
                        watched.keeper.judged_turn();
                    }
                }
                Ok(_) => {}
                // A missed turn is a missed ask: the next pass still sees
                // the card.
                Err(RecvError::Lagged(_)) => {}
                Err(RecvError::Closed) => break,
            }
        }
    });
    let every = app.cfg.keeper.tick_seconds;
    if every == 0 {
        return;
    }
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(every));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        // Not at once: the first pass waits a full interval after boot.
        tick.tick().await;
        loop {
            tick.tick().await;
            if let Err(e) = step(&app, &app.keeper, Utc::now()) {
                tracing::warn!(error = %e, "board keeper pass failed");
            }
        }
    });
}

/// Whether the keeper may act now: on, and no install pause open.
fn awake(app: &AppState) -> anyhow::Result<bool> {
    Ok(app.cfg.keeper.enabled && app.db.open_quiesce()?.is_none())
}

/// One pass over every board this computer is home to, as of `now`, with
/// `keeper`'s memory (the daemon's own, or a test's).
pub fn step(app: &Arc<AppState>, keeper: &Keeper, now: DateTime<Utc>) -> anyhow::Result<()> {
    if !awake(app)? {
        return Ok(());
    }
    let mut budget = Budget::new(app.cfg.keeper.max_messages_per_tick);
    for project in app.db.list_projects()? {
        if project.deleted_at.is_some() {
            continue;
        }
        if let Err(e) = pass(app, keeper, &project, None, now, &mut budget) {
            tracing::warn!(project_id = %project.id, error = %e, "board keeper pass failed");
        }
    }
    Ok(())
}

/// A bot's turn that just ended.
pub struct Turn<'a> {
    pub bot_id: &'a str,
    pub started: DateTime<Utc>,
    /// The card `bot_state` named for it (`supervisor::WorkingOn`): the one
    /// a delivery named, or the last one the bot changed.
    pub card: Option<&'a str>,
}

/// After a turn: the bot's cards are judged again, and each it changed
/// without anyone moving it is asked about (rule 3), the turn's own card
/// first. The card's history is the proof of work: a delivery that named a
/// card the bot then left alone asks nothing.
pub fn after_turn(app: &Arc<AppState>, keeper: &Keeper, turn: &Turn<'_>, now: DateTime<Utc>) {
    if let Err(e) = try_after_turn(app, keeper, turn, now) {
        tracing::warn!(bot_id = turn.bot_id, error = %e, "board keeper turn check failed");
    }
}

fn try_after_turn(
    app: &Arc<AppState>,
    keeper: &Keeper,
    turn: &Turn<'_>,
    now: DateTime<Utc>,
) -> anyhow::Result<()> {
    let bot_id = turn.bot_id;
    if !awake(app)? {
        return Ok(());
    }
    let Some(bot) = app.db.get_live_bot(bot_id)? else {
        return Ok(());
    };
    let Some(project) = app.db.get_project(&bot.project_id)? else {
        return Ok(());
    };
    let mut budget = Budget::new(app.cfg.keeper.max_messages_per_tick);
    let Some(loaded) = pass(app, keeper, &project, Some(bot_id), now, &mut budget)? else {
        return Ok(());
    };
    let mut asks = Budget::for_turn(ASKS_PER_TURN);
    let mut reach = gather::Reach::default();
    let mut worked = app.db.keeper_turn_work(bot_id, turn.started)?;
    worked.sort_by_key(|w| Some(w.item_id.as_str()) != turn.card);
    for work in worked {
        let Some(card) = loaded.card(&work.item_id) else {
            continue;
        };
        let reachable = reach.of(app, &project.id, card.assignee.as_deref());
        let facts = loaded.facts(card, keeper, reachable);
        let marks = loaded.marks_of(card);
        let asked = marks.iter().find(|m| m.rule == Rule::NotMoved.name());
        if let Some(act) = rules::after_turn(&facts, &work, asked, now) {
            let plan = rules::Plan {
                acts: vec![act],
                publish: false,
            };
            let done = apply(app, &loaded, card, plan, &mut asks)?;
            announce::told(app, &loaded, card, &done);
        }
    }
    Ok(())
}

/// Judge the project's cards (only `only_bot`'s, when given) and act. The
/// project as read, when the keeper runs for it here.
fn pass(
    app: &Arc<AppState>,
    keeper: &Keeper,
    project: &bus::Project,
    only_bot: Option<&str>,
    now: DateTime<Utc>,
    budget: &mut Budget,
) -> anyhow::Result<Option<gather::Project>> {
    if !crate::attention::is_home(app, &project.id) {
        return Ok(None);
    }
    let settings = app.cfg.keeper.for_project(&project.name);
    if !settings.enabled {
        app.db.keeper_clear_project(&project.id)?;
        return Ok(None);
    }
    let loaded = gather::load(app, &project.id)?;
    let clock = Clock {
        now,
        boot: keeper.boot(),
    };
    let mut reach = gather::Reach::default();
    for card in &loaded.cards {
        if only_bot.is_some_and(|bot| card.assignee.as_deref() != Some(bot)) {
            continue;
        }
        let reachable = reach.of(app, &project.id, card.assignee.as_deref());
        let facts = loaded.facts(card, keeper, reachable);
        let plan = rules::judge(&facts, &loaded.marks_of(card), clock, &settings.limits);
        if plan.is_empty() {
            continue;
        }
        let done = apply(app, &loaded, card, plan, budget)?;
        announce::told(app, &loaded, card, &done);
    }
    Ok(Some(loaded))
}
