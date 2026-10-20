//! The running cap and the idle time (0.23.0): the config file's values,
//! overridden by `set_config` and persisted in the `meta` table like
//! `auto_compact_window`.

use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::time::Duration;

use crate::config::TeamConfig;
use crate::db::Db;

pub const MAX_RUNNING_META_KEY: &str = "max_running_bots";
pub const IDLE_SLEEP_META_KEY: &str = "idle_sleep_minutes";
pub const MAX_RUNNING_RANGE: std::ops::RangeInclusive<u64> = 1..=32;
pub const IDLE_SLEEP_RANGE: std::ops::RangeInclusive<u64> = 1..=1440;

pub(super) struct TeamSettings {
    max_running: AtomicUsize,
    idle_minutes: AtomicU32,
    /// Seconds in place of minutes, for tests.
    idle_seconds: Option<u64>,
}

impl TeamSettings {
    /// The config's values, then whatever `set_config` stored.
    pub fn load(cfg: &TeamConfig, db: &Db) -> Self {
        let stored = |key: &str, range: &std::ops::RangeInclusive<u64>| {
            db.get_meta(key)
                .ok()
                .flatten()
                .and_then(|v| v.parse::<u64>().ok())
                .filter(|v| range.contains(v))
        };
        let max = stored(MAX_RUNNING_META_KEY, &MAX_RUNNING_RANGE)
            .map_or(cfg.max_running_bots, |v| v as usize);
        let idle = stored(IDLE_SLEEP_META_KEY, &IDLE_SLEEP_RANGE)
            .map_or(cfg.idle_sleep_minutes, |v| v as u32);
        Self {
            max_running: AtomicUsize::new(max.max(1)),
            idle_minutes: AtomicU32::new(idle.max(1)),
            idle_seconds: cfg.idle_sleep_seconds_for_tests,
        }
    }

    pub fn max_running(&self) -> usize {
        self.max_running.load(Ordering::Relaxed)
    }

    pub fn idle_minutes(&self) -> u32 {
        self.idle_minutes.load(Ordering::Relaxed)
    }

    pub fn idle_after(&self) -> Duration {
        match self.idle_seconds {
            Some(seconds) => Duration::from_secs(seconds),
            None => Duration::from_secs(u64::from(self.idle_minutes()) * 60),
        }
    }

    pub fn set_max_running(&self, db: &Db, max: usize) -> anyhow::Result<()> {
        db.set_meta(MAX_RUNNING_META_KEY, &max.to_string())?;
        self.max_running.store(max, Ordering::Relaxed);
        Ok(())
    }

    pub fn set_idle_minutes(&self, db: &Db, minutes: u32) -> anyhow::Result<()> {
        db.set_meta(IDLE_SLEEP_META_KEY, &minutes.to_string())?;
        self.idle_minutes.store(minutes, Ordering::Relaxed);
        Ok(())
    }
}

impl super::Supervisor {
    /// The running cap (`set_config`), persisted; a lower cap lets running
    /// bots finish, and only stops new ones starting.
    pub fn set_max_running(&self, max: usize) -> anyhow::Result<()> {
        let before = self.team_status();
        self.inner
            .team_settings
            .set_max_running(&self.inner.db, max)?;
        self.grant_slots();
        self.push_team_if_changed(&before);
        Ok(())
    }

    pub fn set_idle_sleep_minutes(&self, minutes: u32) -> anyhow::Result<()> {
        let before = self.team_status();
        self.inner
            .team_settings
            .set_idle_minutes(&self.inner.db, minutes)?;
        self.push_team_if_changed(&before);
        Ok(())
    }
}
