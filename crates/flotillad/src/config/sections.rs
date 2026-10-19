//! Sections of the daemon's config (moved out of config.rs for the
//! 400-line limit): retention, delivery and the routine scheduler.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RetentionConfig {
    pub enabled: bool,
    pub message_days: i64,
    pub delivery_days: i64,
    pub routine_run_days: i64,
    /// How long emitted signals are kept once no run references them.
    pub signal_days: i64,
    /// How long identity revisions are kept. This is the undo buffer for
    /// changes bots make to themselves, so it outlives ordinary chat.
    pub revision_days: i64,
    /// How long an archived bot's workspace stays on disk before reclamation.
    pub archived_bot_days: i64,
    /// How often the pruning task runs.
    pub interval_hours: u64,
}

impl Default for RetentionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            message_days: 180,
            delivery_days: 30,
            routine_run_days: 90,
            signal_days: 30,
            revision_days: 365,
            archived_bot_days: 30,
            interval_hours: 24,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DeliveryConfig {
    pub poll_interval_ms: u64,
    pub lease_seconds: i64,
    pub max_attempts: i64,
    pub base_backoff_seconds: i64,
    /// Type the owner's app chat into a bot's composer as their own turn
    /// (H-195 D2 + D2b, one switch). See `bus_auth::composer_delivery`.
    pub composer: bool,
    /// The same, for these bots only (by name or id) while `composer` is
    /// off: the live check on one test bot (H-209). Every other bot keeps
    /// inbox delivery.
    pub composer_bots: Vec<String>,
}

impl Default for DeliveryConfig {
    fn default() -> Self {
        Self {
            poll_interval_ms: 1_000,
            lease_seconds: 60,
            max_attempts: 10,
            base_backoff_seconds: 5,
            composer: false,
            composer_bots: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SchedulerConfig {
    pub tick_interval_ms: u64,
    /// Default deadline for one occurrence, when its routine sets none.
    pub lease_seconds: i64,
    /// How far back to schedule missed occurrences on startup.
    pub catch_up_window_minutes: i64,
    /// First retry delay for a failed occurrence; doubled per attempt.
    pub base_backoff_seconds: i64,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            tick_interval_ms: 30_000,
            lease_seconds: 300,
            catch_up_window_minutes: 60,
            base_backoff_seconds: 30,
        }
    }
}

/// Where a deleted project's leftovers go (0.19.0): the OS trash when there
/// is one, else `<home>/trash/<date>-<project-dir>/`, kept `fallback_days`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TrashConfig {
    /// Use the macOS Trash, the Windows Recycle Bin or the freedesktop
    /// trash. Off: everything goes to the trash folder in the home.
    pub os_trash: bool,
    /// How long the trash folder in the home keeps what it holds; swept
    /// with the daily retention.
    pub fallback_days: i64,
    /// How many times a linked computer is asked, on a live link, to trash
    /// its half of a deleted project before it counts as not confirming.
    pub peer_attempts: i64,
    /// Minutes between those asks while it doesn't answer.
    pub peer_resend_minutes: i64,
    /// How long a delete waits for the project's stopped sessions to let go
    /// of their files before it leaves what they still hold.
    pub held_wait_seconds: u64,
}

impl Default for TrashConfig {
    fn default() -> Self {
        Self {
            os_trash: true,
            fallback_days: 30,
            peer_attempts: 3,
            peer_resend_minutes: 15,
            held_wait_seconds: 10,
        }
    }
}

/// The team on demand (0.23.0): how many bots run at once on this computer,
/// and how long an on-demand bot stays idle before it sleeps. `set_config`
/// overrides both, persisted in the daemon's database.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TeamConfig {
    pub max_running_bots: usize,
    pub idle_sleep_minutes: u32,
    /// Seconds in place of `idle_sleep_minutes`, for tests only: never read
    /// from a config file.
    #[serde(skip)]
    pub idle_sleep_seconds_for_tests: Option<u64>,
}

impl Default for TeamConfig {
    fn default() -> Self {
        Self {
            max_running_bots: 6,
            idle_sleep_minutes: 10,
            idle_sleep_seconds_for_tests: None,
        }
    }
}
