//! The supervisor's side of the team on demand (0.23.0): waking a bot,
//! putting one to sleep, the running cap, and the `team_updated` push.

use std::collections::HashSet;

use bus::{Bot, BotPresence};

use super::team::{Team, TeamStatus, WakeReason, WakeRequest, Woke};
use super::*;

/// The reasons a sleeping or waiting bot shows.
const ASLEEP: &str = "asleep: nothing to do";
const QUEUED: &str = "waiting for a free slot";

impl Supervisor {
    fn lock_team(&self) -> std::sync::MutexGuard<'_, Team> {
        self.inner.team.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The running count and the queue for a slot.
    pub fn team_status(&self) -> TeamStatus {
        let team = self.lock_team();
        TeamStatus {
            running: team.running(),
            max_running: self.inner.team_settings.max_running(),
            idle_sleep_minutes: self.inner.team_settings.idle_minutes(),
            queue: team.queue(),
        }
    }

    pub(super) fn push_team_if_changed(&self, before: &TeamStatus) {
        let now = self.team_status();
        if now != *before {
            self.inner.events.push(Push::TeamUpdated { team: now });
        }
    }

    /// Wakes `bot` for `request`: it starts when a slot is free, else it
    /// queues. `None` for a bot that doesn't run here.
    pub fn wake(&self, bot: &Bot, request: WakeRequest) -> Option<Woke> {
        self.wake_batch(vec![(bot.clone(), request)]).pop()
    }

    /// A slot for a bot its caller starts itself, as creation does: given
    /// one, it isn't started a second time behind the caller's back.
    pub fn take_slot(&self, bot: &Bot, request: WakeRequest) -> Option<Woke> {
        self.wake_all(vec![(bot.clone(), request)], false).pop()
    }

    /// Wakes bots asked for at the same moment, so card work among them
    /// keeps the board's order.
    pub fn wake_batch(&self, wakes: Vec<(Bot, WakeRequest)>) -> Vec<Woke> {
        self.wake_all(wakes, true)
    }

    /// `start_now`: start each bot given a slot at once. The tick leaves its
    /// own to the reconciler, which staggers a boot's starts.
    fn wake_all(&self, wakes: Vec<(Bot, WakeRequest)>, start_now: bool) -> Vec<Woke> {
        let before = self.team_status();
        let max = self.inner.team_settings.max_running();
        let mut asked = Vec::new();
        for (bot, request) in wakes {
            if bot.is_linked() || bot.deleted_at.is_some() {
                continue;
            }
            // On its way to sleep: it wakes again once it is down.
            if self.going_to_sleep(&bot.id) {
                self.lock_team().rewake.insert(bot.id.clone(), request);
                continue;
            }
            asked.push((bot.id, bot.project_id, request));
        }
        let woke = {
            let mut team = self.lock_team();
            for (bot_id, _, _) in &asked {
                team.sleep_requested.remove(bot_id);
                team.idle_since.remove(bot_id);
            }
            team.wake_all(asked, max, chrono::Utc::now())
        };
        for (bot_id, woke) in &woke {
            match woke {
                Woke::Queued => self.show_resting(bot_id, BotState::Queued, QUEUED),
                Woke::Started if start_now => self.start_soon(bot_id),
                Woke::Started => tracing::info!(bot_id, "woken"),
                Woke::Running => {}
            }
        }
        self.push_team_if_changed(&before);
        woke.into_iter().map(|(_, woke)| woke).collect()
    }

    /// Starts a bot just given a slot now, rather than at the next tick;
    /// the reconciler retries a start that fails.
    fn start_soon(&self, bot_id: &str) {
        tracing::info!(bot_id, "woken; starting");
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            return;
        };
        let (supervisor, bot_id) = (self.clone(), bot_id.to_string());
        runtime.spawn_blocking(move || {
            if let Err(e) = supervisor.start_bot(&bot_id) {
                tracing::warn!(bot_id, error = %e, "a woken bot did not start; will retry");
            }
        });
    }

    /// The owner's `sleep_bot`: an on-demand bot sleeps once its running
    /// turn ends; a queued one stops waiting.
    pub fn request_sleep(&self, bot_id: &str) {
        let before = self.team_status();
        let dequeued = {
            let mut team = self.lock_team();
            if team.holds(bot_id) {
                team.sleep_requested.insert(bot_id.to_string());
                false
            } else {
                team.dequeue(bot_id).is_some()
            }
        };
        if dequeued {
            self.show_resting(bot_id, BotState::Asleep, ASLEEP);
        }
        self.push_team_if_changed(&before);
    }

    fn going_to_sleep(&self, bot_id: &str) -> bool {
        self.lock_bots().get(bot_id).is_some_and(|h| h.sleeping)
    }

    /// A state the supervisor sets without a session behind it: asleep or
    /// queued. `set_state` needs a handle; one asleep since boot has none.
    fn show_resting(&self, bot_id: &str, state: BotState, reason: &str) {
        {
            let mut bots = self.lock_bots();
            let h = bots
                .entry(bot_id.to_string())
                .or_insert_with(|| BotHandle::new(self.inner.cfg.scrollback_bytes));
            if h.session.is_some() || h.starting || (h.state == state && h.reason == reason) {
                return;
            }
            h.state = state;
            h.reason = reason.to_string();
        }
        tracing::info!(bot_id, state = state.as_str(), "bot resting");
        self.inner
            .events
            .push(Push::bot_state(bot_id, state, reason));
        self.inner.working.forget(bot_id);
    }

    /// Stops the bot's session into sleep: a clean stop the reconciler
    /// doesn't undo, and no failure.
    fn put_to_sleep(&self, bot_id: &str) {
        let session = {
            let mut bots = self.lock_bots();
            let Some(h) = bots.get_mut(bot_id) else {
                return;
            };
            if h.stopping {
                return;
            }
            h.stopping = true;
            h.sleeping = true;
            h.session.clone()
        };
        tracing::info!(bot_id, "going to sleep");
        match session {
            Some(session) => {
                self.set_state(bot_id, BotState::Stopping, "going to sleep");
                if let Err(e) = session.lock().unwrap_or_else(|e| e.into_inner()).kill() {
                    tracing::warn!(bot_id, error = %e, "could not stop a bot going to sleep");
                }
            }
            None => {
                self.stopped(bot_id);
            }
        }
    }

    /// The session of a bot the daemon stopped exited: asleep, with its slot
    /// given back, when it went to sleep (0.23.0); stopped otherwise.
    pub(super) fn stopped(&self, bot_id: &str) {
        let sleeping = {
            let mut bots = self.lock_bots();
            let Some(h) = bots.get_mut(bot_id) else {
                return;
            };
            let sleeping = std::mem::take(&mut h.sleeping);
            if sleeping {
                h.stopping = false;
            }
            sleeping
        };
        if !sleeping {
            self.set_state(bot_id, BotState::Stopped, "Stopped by the Flotilla service");
            return;
        }
        let before = self.team_status();
        let rewake = {
            let mut team = self.lock_team();
            team.release(bot_id);
            team.slept.insert(bot_id.to_string());
            team.rewake.remove(bot_id)
        };
        self.set_state(bot_id, BotState::Asleep, ASLEEP);
        self.inner.working.forget(bot_id);
        self.push_team_if_changed(&before);
        if let Some(request) = rewake {
            if let Ok(Some(bot)) = self.inner.db.get_live_bot(bot_id) {
                self.wake(&bot, request);
            }
        }
    }

    /// A state change of a running bot: a turn resets its idle clock, and
    /// a session that came up takes its waiting deliveries at once.
    pub(super) fn team_on_state(&self, bot_id: &str, state: BotState) {
        if state == BotState::Ready {
            if let Err(e) = self.inner.db.deliveries_due_now(bot_id) {
                tracing::warn!(bot_id, error = %e, "could not hurry the bot's deliveries");
            }
        } else {
            self.lock_team().idle_since.remove(bot_id);
        }
    }

    /// One supervision tick of the team: who holds a slot, who gets a free
    /// one, who falls asleep. Returns the bots the reconciler may start.
    pub(super) fn team_tick(&self, bots: &[Bot]) -> HashSet<String> {
        let before = self.team_status();
        let local: Vec<&Bot> = bots.iter().filter(|b| !b.is_linked()).collect();
        let live: HashSet<String> = local.iter().map(|b| b.id.clone()).collect();
        let leads = self.leads();
        let always_on =
            |bot: &Bot| bot.presence == BotPresence::AlwaysOn || leads.contains(&bot.id);
        let running: Vec<String> = {
            let handles = self.lock_bots();
            handles
                .iter()
                .filter(|(_, h)| h.session.is_some() || h.starting)
                .map(|(id, _)| id.clone())
                .collect()
        };
        {
            let mut team = self.lock_team();
            team.retain(&live);
            for id in running.iter().filter(|id| live.contains(*id)) {
                team.adopt(id);
            }
        }
        if !self
            .inner
            .booted
            .swap(true, std::sync::atomic::Ordering::SeqCst)
        {
            self.wake_busy(&local, &always_on);
        }
        let asks: Vec<(Bot, WakeRequest)> = local
            .iter()
            .filter(|b| always_on(b))
            .filter(|b| {
                let team = self.lock_team();
                !team.holds(&b.id) && !team.is_queued(&b.id)
            })
            .map(|b| {
                let reason = if leads.contains(&b.id) {
                    WakeReason::Lead
                } else {
                    WakeReason::Owner
                };
                ((*b).clone(), WakeRequest::new(reason))
            })
            .collect();
        if !asks.is_empty() {
            self.wake_all(asks, false);
        }
        self.grant_slots();
        for bot in local.iter().filter(|b| !always_on(b)) {
            self.tend_on_demand(&bot.id);
        }
        self.push_team_if_changed(&before);
        let team = self.lock_team();
        live.into_iter().filter(|id| team.holds(id)).collect()
    }

    /// Gives free slots to the head of the queue. The time each waited is
    /// added to its open tasks' deadlines.
    pub(super) fn grant_slots(&self) {
        let started = self
            .lock_team()
            .grant(self.inner.team_settings.max_running());
        for (bot_id, since) in started {
            let waited = chrono::Duration::from_std(since.elapsed()).unwrap_or_default();
            if let Err(e) = self.inner.db.extend_bot_task_deadlines(&bot_id, waited) {
                tracing::warn!(bot_id, error = %e, "could not extend the bot's deadlines");
            }
            tracing::info!(bot_id, waited = waited.num_seconds(), "a slot freed up");
        }
    }

    /// An on-demand bot: asleep when it holds no slot; to sleep when the
    /// owner asked and its turn is over, or when it has been idle long
    /// enough.
    fn tend_on_demand(&self, bot_id: &str) {
        let (holds, queued, requested) = {
            let team = self.lock_team();
            let requested = team.sleep_requested.contains(bot_id);
            (team.holds(bot_id), team.is_queued(bot_id), requested)
        };
        if !holds {
            if !queued {
                self.show_resting(bot_id, BotState::Asleep, ASLEEP);
            }
            return;
        }
        let (state, up) = {
            let bots = self.lock_bots();
            match bots.get(bot_id) {
                Some(h) => (h.state, h.session.is_some() && !h.stopping),
                None => return,
            }
        };
        if !up {
            return;
        }
        if requested && !working_on::in_turn(state) && state != BotState::Starting {
            self.put_to_sleep(bot_id);
            return;
        }
        let idle = state == BotState::Ready && !self.inner.db.bot_has_work(bot_id).unwrap_or(true);
        let since = {
            let mut team = self.lock_team();
            if !idle {
                team.idle_since.remove(bot_id);
                return;
            }
            *team
                .idle_since
                .entry(bot_id.to_string())
                .or_insert_with(Instant::now)
        };
        if since.elapsed() >= self.inner.team_settings.idle_after() {
            self.put_to_sleep(bot_id);
        }
    }

    /// At boot: an on-demand bot that still has work (an open task, a card
    /// in an active column) wakes to pick it back up.
    fn wake_busy(&self, local: &[&Bot], always_on: &dyn Fn(&Bot) -> bool) {
        let mut wakes = Vec::new();
        for bot in local.iter().filter(|b| !always_on(b)) {
            let Ok(Some(work)) = self.inner.db.bot_open_work(&bot.id) else {
                continue;
            };
            let (reason, item) = match work {
                crate::db::OpenWork::Task(item) => (WakeReason::Task, item),
                crate::db::OpenWork::Card(item) => (WakeReason::Card, Some(item)),
            };
            let request = super::team_wake::request(&self.inner.db, reason, item.as_deref());
            wakes.push(((*bot).clone(), request));
        }
        if !wakes.is_empty() {
            self.wake_all(wakes, false);
        }
    }

    /// Every project's lead, always on whatever its row says.
    fn leads(&self) -> HashSet<String> {
        match self.inner.db.list_projects() {
            Ok(projects) => projects.into_iter().filter_map(|p| p.lead_bot_id).collect(),
            Err(e) => {
                tracing::warn!(error = %e, "could not read the projects' leads");
                HashSet::new()
            }
        }
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
