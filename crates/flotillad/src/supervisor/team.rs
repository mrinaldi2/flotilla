//! The team on demand (0.23.0): which bots hold one of this computer's
//! running slots, and the queue of wakes waiting for one.
//!
//! A bot runs only while it holds a slot. An always-on bot asks for one
//! whenever it has none; an on-demand bot is woken by work (a delivery, a
//! routine, the owner) and gives its slot back when it falls asleep. At most
//! `max_running_bots` bots hold a slot at once. A wake with none free
//! queues, in the order the owner decided: the owner's own wakes, then the
//! lead, then card work in the board's order as of when it queued (an
//! urgent bug at its front), then everything else as it came.

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use chrono::{DateTime, Utc};
use serde::Serialize;

/// Why a bot was woken, which sets its place in the queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WakeReason {
    Owner,
    Lead,
    Card,
    Task,
    Message,
    Review,
    Routine,
}

/// What a wake is about: its reason, and the card it names, read when it
/// was asked for.
#[derive(Debug, Clone, Default)]
pub struct WakeCard {
    pub item_id: String,
    /// `P0` to `P3`.
    pub priority: String,
    /// A P0 bug.
    pub urgent: bool,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct WakeRequest {
    pub reason: WakeReason,
    pub card: Option<WakeCard>,
    /// An on-demand bot's wake, which its project's budget holds (0.24.0);
    /// the supervisor sets it. A lead's or an always-on bot's never is.
    pub budgeted: bool,
}

impl WakeRequest {
    pub fn new(reason: WakeReason) -> Self {
        Self {
            reason,
            card: None,
            budgeted: false,
        }
    }

    fn class(&self) -> u8 {
        match (self.reason, &self.card) {
            (WakeReason::Owner, _) => 0,
            (WakeReason::Lead, _) => 1,
            (_, Some(_)) => 2,
            (_, None) => 3,
        }
    }
}

/// A queued wake, as `team_status` lists it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TeamWake {
    pub bot_id: String,
    pub project_id: String,
    pub reason: WakeReason,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<String>,
    pub urgent: bool,
    pub queued_at: DateTime<Utc>,
    /// `budget`: its project used its weekly budget, so it waits until the
    /// owner raises or extends it (0.24.0, `usage_v1`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub held: Option<&'static str>,
}

/// This computer's running bots and the queue for a slot.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TeamStatus {
    pub running: usize,
    pub max_running: usize,
    pub idle_sleep_minutes: u32,
    pub queue: Vec<TeamWake>,
}

#[derive(Debug, Clone)]
struct Queued {
    bot_id: String,
    project_id: String,
    request: WakeRequest,
    queued_at: DateTime<Utc>,
    since: Instant,
    /// The wakes asked for together share a batch: card work keeps the
    /// board's order within one, and batches keep the order they came in.
    batch: u64,
    seq: u64,
}

impl Queued {
    /// Lower starts first.
    fn key(&self) -> (u8, bool, u64, u8, Option<DateTime<Utc>>, u64) {
        let class = self.request.class();
        match &self.request.card {
            Some(card) if class == 2 => (
                class,
                !card.urgent,
                self.batch,
                priority_rank(&card.priority),
                card.created_at,
                self.seq,
            ),
            _ => (class, true, self.seq, 0, None, self.seq),
        }
    }

    fn view(&self, held: bool) -> TeamWake {
        let card = self.request.card.as_ref();
        TeamWake {
            bot_id: self.bot_id.clone(),
            project_id: self.project_id.clone(),
            reason: self.request.reason,
            item_id: card.map(|c| c.item_id.clone()),
            priority: card.map(|c| c.priority.clone()),
            urgent: card.is_some_and(|c| c.urgent),
            queued_at: self.queued_at,
            held: held.then_some("budget"),
        }
    }
}

fn priority_rank(priority: &str) -> u8 {
    match priority {
        "P0" => 0,
        "P1" => 1,
        "P2" => 2,
        _ => 3,
    }
}

/// What became of a wake.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Woke {
    /// It already holds a slot.
    Running,
    /// It was given a free slot.
    Started,
    /// It waits for one.
    Queued,
    /// It waits until its project's budget is raised or extended (0.24.0).
    Held,
}

/// The slots and the queue. Kept by the supervisor; every method is a plain
/// state change, so the ordering is tested without a runtime.
#[derive(Default)]
pub(super) struct Team {
    /// Bots holding a running slot.
    holders: HashSet<String>,
    queue: Vec<Queued>,
    /// On-demand holders idle since, by bot.
    pub(super) idle_since: HashMap<String, Instant>,
    /// Bots the owner sent to sleep once their turn ends.
    pub(super) sleep_requested: HashSet<String>,
    /// Wakes that came while the bot was on its way to sleep.
    pub(super) rewake: HashMap<String, WakeRequest>,
    /// Bots that went to sleep cleanly and haven't started since.
    pub(super) slept: HashSet<String>,
    /// Projects whose on-demand bots' wakes are held for the budget.
    held: HashSet<String>,
    next: u64,
}

impl Team {
    pub fn holds(&self, bot_id: &str) -> bool {
        self.holders.contains(bot_id)
    }

    pub fn is_queued(&self, bot_id: &str) -> bool {
        self.queue.iter().any(|q| q.bot_id == bot_id)
    }

    pub fn running(&self) -> usize {
        self.holders.len()
    }

    /// Holds the on-demand bots' wakes of these projects, and releases the
    /// rest. Whether that changed anything.
    pub fn set_held(&mut self, held: HashSet<String>) -> bool {
        let changed = held != self.held;
        self.held = held;
        changed
    }

    fn is_held(&self, project_id: &str, request: &WakeRequest) -> bool {
        request.budgeted && self.held.contains(project_id)
    }

    /// A queued wake that could start once a slot frees.
    fn startable(&self, q: &Queued) -> bool {
        !self.is_held(&q.project_id, &q.request)
    }

    /// A bot that runs without having asked (it was started directly)
    /// holds a slot all the same.
    pub fn adopt(&mut self, bot_id: &str) {
        if self.holders.insert(bot_id.to_string()) {
            self.queue.retain(|q| q.bot_id != bot_id);
        }
    }

    pub fn release(&mut self, bot_id: &str) {
        self.holders.remove(bot_id);
        self.idle_since.remove(bot_id);
        self.sleep_requested.remove(bot_id);
    }

    /// Drops a queued wake; the time it waited.
    pub fn dequeue(&mut self, bot_id: &str) -> Option<Instant> {
        let at = self.queue.iter().position(|q| q.bot_id == bot_id)?;
        Some(self.queue.remove(at).since)
    }

    /// Forgets every bot `keep` doesn't name.
    pub fn retain(&mut self, keep: &HashSet<String>) {
        self.holders.retain(|id| keep.contains(id));
        self.queue.retain(|q| keep.contains(&q.bot_id));
        self.idle_since.retain(|id, _| keep.contains(id));
        self.sleep_requested.retain(|id| keep.contains(id));
        self.rewake.retain(|id, _| keep.contains(id));
        self.slept.retain(|id| keep.contains(id));
    }

    /// Wakes the bots of one batch, asked for at the same moment.
    pub fn wake_all(
        &mut self,
        wakes: Vec<(String, String, WakeRequest)>,
        max: usize,
        now: DateTime<Utc>,
    ) -> Vec<(String, Woke)> {
        self.next += 1;
        let batch = self.next;
        wakes
            .into_iter()
            .map(|(bot_id, project_id, request)| {
                let woke = self.wake_in(batch, &bot_id, &project_id, request, max, now);
                (bot_id, woke)
            })
            .collect()
    }

    #[cfg(test)]
    pub fn wake(
        &mut self,
        bot_id: &str,
        project_id: &str,
        request: WakeRequest,
        max: usize,
        now: DateTime<Utc>,
    ) -> Woke {
        self.next += 1;
        self.wake_in(self.next, bot_id, project_id, request, max, now)
    }

    fn wake_in(
        &mut self,
        batch: u64,
        bot_id: &str,
        project_id: &str,
        request: WakeRequest,
        max: usize,
        now: DateTime<Utc>,
    ) -> Woke {
        if self.holds(bot_id) {
            return Woke::Running;
        }
        let held = self.is_held(project_id, &request);
        let waits = if held { Woke::Held } else { Woke::Queued };
        if let Some(queued) = self.queue.iter_mut().find(|q| q.bot_id == bot_id) {
            // Already waiting: only the owner, or an urgent bug, moves it up.
            let urgent = request.card.as_ref().is_some_and(|c| c.urgent);
            if request.class() < queued.request.class()
                || (urgent && !queued.request.card.as_ref().is_some_and(|c| c.urgent))
            {
                queued.request = request;
                queued.batch = batch;
            }
            return waits;
        }
        let free = !self.queue.iter().any(|q| self.startable(q)) && self.holders.len() < max;
        if !held && free {
            self.holders.insert(bot_id.to_string());
            return Woke::Started;
        }
        self.next += 1;
        self.queue.push(Queued {
            bot_id: bot_id.to_string(),
            project_id: project_id.to_string(),
            request,
            queued_at: now,
            since: Instant::now(),
            batch,
            seq: self.next,
        });
        waits
    }

    /// Gives free slots to the head of the queue: each bot started, and how
    /// long it waited.
    pub fn grant(&mut self, max: usize) -> Vec<(String, Instant)> {
        let mut started = Vec::new();
        while self.holders.len() < max {
            let Some(head) = self.head() else {
                break;
            };
            let queued = self.queue.remove(head);
            self.holders.insert(queued.bot_id.clone());
            started.push((queued.bot_id, queued.since));
        }
        started
    }

    fn head(&self) -> Option<usize> {
        (0..self.queue.len())
            .filter(|&i| self.startable(&self.queue[i]))
            .min_by_key(|&i| self.queue[i].key())
    }

    /// The queue in the order it will start.
    pub fn queue(&self) -> Vec<TeamWake> {
        let mut queue: Vec<&Queued> = self.queue.iter().collect();
        queue.sort_by_key(|q| q.key());
        queue
            .into_iter()
            .map(|q| q.view(!self.startable(q)))
            .collect()
    }
}

#[cfg(test)]
#[path = "team_tests.rs"]
mod tests;
