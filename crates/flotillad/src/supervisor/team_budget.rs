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
