//! The team's side of usage and budgets (0.24.0, `usage_v1`): holding a
//! project's on-demand bots while it is at its weekly budget, and the
//! questions others ask of a bot's rest.

use std::collections::HashSet;

use super::*;

impl Supervisor {
    /// Holds the on-demand bots' wakes of `held` projects, and releases
    /// the rest: a released wake takes a free slot, and the reconciler
    /// starts it at the next tick. The lead and always-on bots never wait
    /// on a budget.
    pub fn set_budget_held(&self, held: HashSet<String>) {
        let before = self.team_status();
        let changed = self.lock_team().set_held(held);
        if changed {
            self.grant_slots();
        }
        self.push_team_if_changed(&before);
    }

    /// Whether the session about to start wakes the bot from a clean sleep,
    /// which left nothing unfinished to be told about; asked once per start.
    pub fn waking_from_sleep(&self, bot_id: &str) -> bool {
        self.lock_team().slept.remove(bot_id)
    }

    /// Asleep or waiting for a slot: the keeper leaves such a bot be.
    pub fn resting(&self, bot_id: &str) -> bool {
        matches!(self.state(bot_id).0, BotState::Asleep | BotState::Queued)
    }
}

impl Supervisor {
    /// What is known of the bot's runtime process, whose background
    /// commands die with it (0.24.0): an asleep or queued bot has none, and
    /// a running one started when its last start did, even when it resumed
    /// the same conversation. A stopped or crashed one is left unknown: a
    /// session can outlive the daemon that started it.
    pub fn runtime(&self, bot_id: &str) -> crate::chat::commands::Runtime {
        use crate::chat::commands::Runtime;
        let bots = self.lock_bots();
        let Some(h) = bots.get(bot_id) else {
            return Runtime::Unknown;
        };
        match h.state {
            BotState::Asleep | BotState::Queued => Runtime::Gone,
            BotState::Stopped | BotState::Crashed | BotState::AuthFailed | BotState::Stopping => {
                Runtime::Unknown
            }
            _ => chrono::Duration::from_std(h.last_start.elapsed())
                .map_or(Runtime::Unknown, |age| {
                    Runtime::Since(chrono::Utc::now() - age)
                }),
        }
    }
}
