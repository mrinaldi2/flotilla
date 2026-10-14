//! `[keeper]` in the daemon's config: on or off, how often it looks, and how
//! long each condition lasts before it acts, with overrides per project
//! (by name, as `[keeper.projects."Shop"]`).

use std::collections::BTreeMap;

use chrono::Duration;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct KeeperConfig {
    pub enabled: bool,
    /// Seconds between passes over every board; 0 leaves only the pass
    /// after each turn of a bot.
    pub tick_seconds: u64,
    /// A card in its work column whose assignee had no turn and nothing
    /// happened on it for this long: the assignee is asked about it once.
    pub idle_nudge_minutes: u64,
    /// The same, for this long: the card is marked stale and the owner gets
    /// a Needs-you row. Counted from the same quiet start.
    pub idle_needs_you_minutes: u64,
    /// An assigned card its assignee hasn't touched for this long: the
    /// assignment notice is sent again, once.
    pub not_started_minutes: u64,
    /// A refused move still refused for this long: a Needs-you row naming
    /// the rule.
    pub blocked_minutes: u64,
    /// Messages to bots one pass may send; the rest wait for the next.
    pub max_messages_per_tick: usize,
    /// Overrides by project name.
    pub projects: BTreeMap<String, KeeperOverride>,
}

/// One project's differences from `[keeper]`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct KeeperOverride {
    pub enabled: Option<bool>,
    pub idle_nudge_minutes: Option<u64>,
    pub idle_needs_you_minutes: Option<u64>,
    pub not_started_minutes: Option<u64>,
    pub blocked_minutes: Option<u64>,
}

impl Default for KeeperConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            tick_seconds: 300,
            idle_nudge_minutes: 120,
            idle_needs_you_minutes: 360,
            not_started_minutes: 30,
            blocked_minutes: 60,
            max_messages_per_tick: 5,
            projects: BTreeMap::new(),
        }
    }
}

/// How long each condition lasts before the keeper acts on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub nudge_after: Duration,
    pub needs_you_after: Duration,
    pub not_started_after: Duration,
    pub blocked_after: Duration,
}

/// What applies to one project.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ForProject {
    pub enabled: bool,
    pub limits: Limits,
}

fn minutes(n: u64) -> Duration {
    // At least a minute, and nothing chrono can't hold.
    Duration::try_minutes(i64::try_from(n.max(1)).unwrap_or(i64::MAX / 120_000))
        .unwrap_or(Duration::MAX)
}

impl KeeperConfig {
    pub fn for_project(&self, name: &str) -> ForProject {
        let o = self
            .projects
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, o)| o.clone())
            .unwrap_or_default();
        let nudge = minutes(o.idle_nudge_minutes.unwrap_or(self.idle_nudge_minutes));
        let needs_you = minutes(
            o.idle_needs_you_minutes
                .unwrap_or(self.idle_needs_you_minutes),
        );
        ForProject {
            enabled: self.enabled && o.enabled.unwrap_or(true),
            limits: Limits {
                nudge_after: nudge,
                // Never before the nudge it follows.
                needs_you_after: needs_you.max(nudge),
                not_started_after: minutes(
                    o.not_started_minutes.unwrap_or(self.not_started_minutes),
                ),
                blocked_after: minutes(o.blocked_minutes.unwrap_or(self.blocked_minutes)),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_and_a_projects_override() {
        let mut cfg: KeeperConfig = toml::from_str(
            r#"
            idle_nudge_minutes = 60
            [projects.Shop]
            idle_nudge_minutes = 10
            idle_needs_you_minutes = 5
            [projects.Quiet]
            enabled = false
            "#,
        )
        .unwrap();
        assert!(cfg.enabled);
        assert_eq!(cfg.tick_seconds, 300);
        let base = cfg.for_project("Other");
        assert_eq!(base.limits.nudge_after, Duration::minutes(60));
        assert_eq!(base.limits.needs_you_after, Duration::minutes(360));
        assert_eq!(base.limits.not_started_after, Duration::minutes(30));
        assert_eq!(base.limits.blocked_after, Duration::minutes(60));
        let shop = cfg.for_project("shop");
        assert_eq!(shop.limits.nudge_after, Duration::minutes(10));
        // Never before the nudge.
        assert_eq!(shop.limits.needs_you_after, Duration::minutes(10));
        assert!(!cfg.for_project("Quiet").enabled);
        cfg.enabled = false;
        assert!(!cfg.for_project("Shop").enabled);
    }

    #[test]
    fn a_zero_or_huge_threshold_stays_usable() {
        let cfg = KeeperConfig {
            not_started_minutes: 0,
            blocked_minutes: u64::MAX,
            ..KeeperConfig::default()
        };
        let p = cfg.for_project("x");
        assert_eq!(p.limits.not_started_after, Duration::minutes(1));
        assert!(p.limits.blocked_after > Duration::days(365));
    }
}
