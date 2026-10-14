//! One kickoff per discussion, even on a double click (0.19.0, after K5).
//!
//! Every invite of a bot to a draft sends it the draft as a kickoff
//! (`draft_talk`), so the owner can reopen a discussion. A double click on
//! "Discuss with a bot", or a phone and a desktop opening the same
//! discussion together, would send the bot the same kickoff twice. So the
//! daemon remembers, per (draft, bot), the last kickoff it sent: what the
//! draft was then (its version and its newest revision) and when. A kickoff
//! for the same draft, unchanged, within [`WINDOW`] is skipped.
//!
//! - An edit, an attachment or a link changes the stamp, so the bot hears
//!   the draft again at once.
//! - Uninviting forgets the bot, so a discussion reopened after it starts
//!   with a fresh kickoff.
//! - Only a kickoff sent restarts the window; skipped ones don't, so a
//!   reopen after the window always sends.
//!
//! It is memory, not the database: it only debounces a burst, a restart
//! forgets nothing worth keeping (at worst one extra kickoff), and every
//! client's invite runs on the board's own computer, so one memory sees
//! them all.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// How long a kickoff covers the same draft, unchanged, for the same bot.
pub const WINDOW: Duration = Duration::from_secs(30);

/// What a kickoff described: the draft's version and its newest revision.
/// Attachments and links write a revision without a new version, so both
/// are needed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stamp {
    pub version: u64,
    pub revision: i64,
}

#[derive(Debug, Clone, Copy)]
struct Sent {
    stamp: Stamp,
    at: Instant,
}

/// The kickoffs sent lately, by (draft, bot).
pub struct Kickoffs {
    sent: Mutex<HashMap<(String, String), Sent>>,
    window_ms: AtomicU64,
}

impl Default for Kickoffs {
    fn default() -> Self {
        Self {
            sent: Mutex::default(),
            window_ms: AtomicU64::new(millis(WINDOW)),
        }
    }
}

fn millis(d: Duration) -> u64 {
    u64::try_from(d.as_millis()).unwrap_or(u64::MAX)
}

impl Kickoffs {
    fn lock(&self) -> MutexGuard<'_, HashMap<(String, String), Sent>> {
        self.sent.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn window(&self) -> Duration {
        Duration::from_millis(self.window_ms.load(Ordering::Relaxed))
    }

    /// Whether to send `bot` a kickoff for `item` as `stamp` describes it,
    /// now. `true` records it as sent, in the same step, so of two invites
    /// at once only one sends.
    pub fn claim(&self, item: &str, bot: &str, stamp: Stamp) -> bool {
        self.claim_at(item, bot, stamp, Instant::now())
    }

    fn claim_at(&self, item: &str, bot: &str, stamp: Stamp, now: Instant) -> bool {
        let window = self.window();
        let mut sent = self.lock();
        // Nothing older than the window matters: keep the map small.
        sent.retain(|_, s| now.saturating_duration_since(s.at) < window);
        let key = (item.to_string(), bot.to_string());
        if sent.get(&key).is_some_and(|s| s.stamp == stamp) {
            return false;
        }
        sent.insert(key, Sent { stamp, at: now });
        true
    }

    /// Forget the kickoff `bot` had for `item`: it was uninvited, or the
    /// kickoff never went out.
    pub fn forget(&self, item: &str, bot: &str) {
        self.lock().remove(&(item.to_string(), bot.to_string()));
    }

    /// A shorter window, so a test can wait it out.
    #[cfg(any(test, feature = "test-support"))]
    pub fn window_for_tests(&self, window: Duration) {
        self.window_ms.store(millis(window), Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: Stamp = Stamp {
        version: 3,
        revision: 7,
    };

    #[test]
    fn the_same_draft_within_the_window_is_sent_once() {
        let k = Kickoffs::default();
        let t = Instant::now();
        assert!(k.claim_at("RB-1", "lead", A, t));
        assert!(!k.claim_at("RB-1", "lead", A, t + Duration::from_secs(1)));
        // Another bot, or another draft, has its own.
        assert!(k.claim_at("RB-1", "builder", A, t));
        assert!(k.claim_at("RB-2", "lead", A, t));
    }

    #[test]
    fn a_change_the_window_or_forgetting_sends_again() {
        let k = Kickoffs::default();
        let t = Instant::now();
        assert!(k.claim_at("RB-1", "lead", A, t));
        let edited = Stamp { version: 4, ..A };
        let attached = Stamp { revision: 8, ..A };
        assert!(k.claim_at("RB-1", "lead", edited, t));
        assert!(k.claim_at("RB-1", "lead", attached, t));
        assert!(k.claim_at("RB-1", "lead", attached, t + WINDOW));
        k.forget("RB-1", "lead");
        assert!(k.claim_at("RB-1", "lead", attached, t + WINDOW));
    }

    #[test]
    fn a_skipped_kickoff_does_not_restart_the_window() {
        let k = Kickoffs::default();
        let t = Instant::now();
        assert!(k.claim_at("RB-1", "lead", A, t));
        assert!(!k.claim_at("RB-1", "lead", A, t + WINDOW / 2));
        assert!(k.claim_at("RB-1", "lead", A, t + WINDOW));
    }
}
