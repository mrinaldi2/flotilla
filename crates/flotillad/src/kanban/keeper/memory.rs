//! What the keeper knows only while the daemon runs: when it started, each
//! bot's turns (from `bot_state`), and the cards a bot read. A restart
//! forgets it; every timer then counts from the start, so nothing that came
//! due while the daemon was down fires at once.

use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::Mutex;

use bus::BotState;
use chrono::{DateTime, Utc};

#[derive(Debug, Default, Clone)]
struct Turns {
    /// When the running turn began; none between turns.
    started: Option<DateTime<Utc>>,
    /// When the bot's last turn began or ended.
    last: Option<DateTime<Utc>>,
}

#[derive(Debug, Default)]
struct Memory {
    bots: HashMap<String, Turns>,
    /// (bot, card) → when the bot last read it.
    reads: HashMap<(String, String), DateTime<Utc>>,
}

/// The keeper's memory of this run.
#[derive(Debug)]
pub struct Keeper {
    /// When this run began, in milliseconds since the epoch.
    boot: AtomicI64,
    memory: Mutex<Memory>,
    /// Turn ends judged, for tests to wait on.
    judged: AtomicU64,
}

impl Default for Keeper {
    fn default() -> Self {
        Self::booted_at(Utc::now())
    }
}

/// A state the bot is in while a turn runs: a prompt or approval wait
/// inside a turn doesn't end it.
pub fn in_turn(state: BotState) -> bool {
    matches!(
        state,
        BotState::Working | BotState::WaitingForApproval | BotState::WaitingForUser
    )
}

/// The most cards one bot's reads are remembered for.
const READS_PER_BOT: usize = 256;

impl Keeper {
    /// A keeper that started at `boot`: a fresh daemon's, or a test's.
    pub fn booted_at(boot: DateTime<Utc>) -> Self {
        Self {
            boot: AtomicI64::new(boot.timestamp_millis()),
            memory: Mutex::default(),
            judged: AtomicU64::new(0),
        }
    }

    pub fn boot(&self) -> DateTime<Utc> {
        DateTime::from_timestamp_millis(self.boot.load(Ordering::SeqCst)).unwrap_or_default()
    }

    /// Tests only: the daemon has been running since `boot`.
    #[cfg(any(test, feature = "test-support"))]
    pub fn running_since_for_tests(&self, boot: DateTime<Utc>) {
        self.boot.store(boot.timestamp_millis(), Ordering::SeqCst);
    }

    /// How many turn ends the keeper has judged this run.
    pub fn turns_judged(&self) -> u64 {
        self.judged.load(Ordering::SeqCst)
    }

    pub(super) fn judged_turn(&self) {
        self.judged.fetch_add(1, Ordering::SeqCst);
    }

    fn with<T>(&self, f: impl FnOnce(&mut Memory) -> T) -> T {
        f(&mut self.memory.lock().unwrap_or_else(|e| e.into_inner()))
    }

    /// The bot is now in `state`, as of `at`. When that ends a turn, the
    /// turn's start, if the keeper saw it.
    pub fn on_state(
        &self,
        bot_id: &str,
        state: BotState,
        at: DateTime<Utc>,
    ) -> Option<DateTime<Utc>> {
        self.with(|m| {
            let turns = m.bots.entry(bot_id.to_string()).or_default();
            if in_turn(state) {
                if turns.started.is_none() {
                    turns.started = Some(at);
                    turns.last = Some(at);
                }
                return None;
            }
            turns.last = Some(at);
            turns.started.take()
        })
    }

    /// Whether the bot is in a turn now.
    pub fn busy(&self, bot_id: &str) -> bool {
        self.with(|m| m.bots.get(bot_id).is_some_and(|t| t.started.is_some()))
    }

    /// When the bot's last turn began or ended, this run.
    pub fn last_turn(&self, bot_id: &str) -> Option<DateTime<Utc>> {
        self.with(|m| m.bots.get(bot_id).and_then(|t| t.last))
    }

    /// The bot read the card (`item_get`): reading a card is touching it.
    pub fn read(&self, bot_id: &str, item_id: &str, at: DateTime<Utc>) {
        self.with(|m| {
            let mine = m.reads.keys().filter(|(b, _)| b == bot_id).count();
            if mine >= READS_PER_BOT {
                let oldest = m
                    .reads
                    .iter()
                    .filter(|((b, _), _)| b == bot_id)
                    .min_by_key(|(_, at)| **at)
                    .map(|(k, _)| k.clone());
                if let Some(oldest) = oldest {
                    m.reads.remove(&oldest);
                }
            }
            m.reads
                .insert((bot_id.to_string(), item_id.to_string()), at);
        });
    }

    /// When the bot last read the card, this run.
    pub fn read_at(&self, bot_id: &str, item_id: &str) -> Option<DateTime<Utc>> {
        self.with(|m| {
            m.reads
                .get(&(bot_id.to_string(), item_id.to_string()))
                .copied()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use BotState::{Ready, WaitingForApproval, Working};

    #[test]
    fn a_turn_is_from_working_to_ready() {
        let t0 = Utc::now();
        let k = Keeper::booted_at(t0);
        assert!(!k.busy("b"));
        assert_eq!(k.on_state("b", Working, t0), None);
        assert!(k.busy("b"));
        // An approval inside the turn doesn't end it, nor move its start.
        let t1 = t0 + Duration::seconds(5);
        assert_eq!(k.on_state("b", WaitingForApproval, t1), None);
        let t2 = t0 + Duration::seconds(9);
        assert_eq!(k.on_state("b", Ready, t2), Some(t0));
        assert!(!k.busy("b"));
        assert_eq!(k.last_turn("b"), Some(t2));
        // Ready again ends nothing.
        assert_eq!(k.on_state("b", Ready, t2), None);
    }

    #[test]
    fn reads_are_remembered_per_bot_and_card() {
        let t0 = Utc::now();
        let k = Keeper::booted_at(t0);
        k.read("b", "P-1", t0);
        assert_eq!(k.read_at("b", "P-1"), Some(t0));
        assert_eq!(k.read_at("c", "P-1"), None);
        for n in 0..READS_PER_BOT + 3 {
            k.read("b", &format!("Q-{n}"), t0 + Duration::seconds(n as i64 + 1));
        }
        assert_eq!(k.read_at("b", "P-1"), None, "the oldest goes first");
        assert!(k
            .read_at("b", &format!("Q-{}", READS_PER_BOT + 2))
            .is_some());
    }
}
