//! Bots whose session isn't running (0.20.0, `attention_bot_stopped`): one
//! `bot_stopped` row per bot of the project here that has been down a
//! minute, crashed, unable to sign in or rate limited, or stopped, never one
//! asleep or queued for a slot. A bot
//! the supervisor brings straight back never gets one, and none is raised
//! while projects are paused for an install. Stand-ins run elsewhere: their
//! computer raises theirs.

use std::time::Duration;

use bus::contract::home::{attention_row::Target, AttentionKind, BotRef};
use bus::BotState;

use super::rows::{Builder, Part};
use super::weight;
use crate::app::AppState;

/// How long a bot is down before it gets its row: past a restart, a boot's
/// staggered starts and the first crash backoffs.
pub(crate) const STOPPED_AFTER: Duration = Duration::from_secs(60);

pub(super) fn stopped_bots(app: &AppState, b: &mut Builder<'_>) -> anyhow::Result<()> {
    if app.db.open_quiesce()?.is_some() {
        return Ok(());
    }
    let now = chrono::Utc::now();
    let grace = chrono::Duration::from_std(STOPPED_AFTER)?;
    let mut bots = app.db.list_bots(Some(b.project_id))?;
    bots.sort_by(|x, y| x.name.cmp(&y.name));
    for bot in bots.iter().filter(|bot| !bot.is_linked()) {
        let (state, reason) = app.supervisor.state(&bot.id);
        // An on-demand bot asleep or waiting for a slot (0.23.0) is resting,
        // not down: it wakes when there is work, so it needs nothing from
        // the owner (0.24.0).
        if matches!(state, BotState::Asleep | BotState::Queued) {
            continue;
        }
        let Some(down) = app.overview.down(&bot.id, state, bot.created_at) else {
            continue;
        };
        if now - down.since < grace {
            continue;
        }
        let what = match down.state {
            BotState::Crashed => "crashed",
            BotState::AuthFailed => "can't sign in",
            BotState::RateLimited => "is rate limited",
            _ => "stopped",
        };
        let reason = reason.trim();
        let title = if reason.is_empty() {
            format!("{} {what}", bot.name)
        } else {
            format!("{} {what}: {reason}", bot.name)
        };
        let part = Part {
            kind: AttentionKind::BotStopped,
            target_id: bot.id.clone(),
            title,
            created_at: down.since,
            target: Some(Target::Bot(BotRef {
                daemon_id: b.me.clone(),
                bot_id: bot.id.clone(),
                name: bot.name.clone(),
            })),
        };
        let row = b.push(part, weight(AttentionKind::BotStopped), None);
        row.bot_state = down.state.as_str().to_string();
    }
    Ok(())
}
