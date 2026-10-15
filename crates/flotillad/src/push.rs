//! Phone push notifications (design only, 0.19.0): what the daemon needs to
//! reach a phone whose app iOS has suspended. Nothing sends yet; push is off
//! until `[push]` names a key, and even then this build only says so.
//!
//! What a phone gets today is the WebSocket: the `board_activity`, `notify`
//! and permission pushes, which the app turns into local notifications while
//! iOS lets it run, a few seconds after it leaves the screen. A suspended
//! app's socket is gone, so nothing reaches it.
//!
//! The plan: the daemon calls Apple's push service itself, outbound HTTPS from
//! the owner's computer, signed with the owner's own provider key, for each
//! paired device whose app registered a push token and has no live
//! connection. No public endpoint is needed, as APNs is only ever called.
//! Every name the product, the app or Apple ties it to is configuration, so
//! nothing here changes when the product is renamed:
//!
//! ```toml
//! [push]
//! topic = ""     # the app's bundle id, which APNs calls the topic
//! key_id = ""    # the provider key's id, from the developer account
//! team_id = ""   # the developer team that owns the key and the app
//! key_path = ""  # the .p8 file; kept out of the daemon's home backups
//! sandbox = false  # true for development builds of the app
//! ```

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// `[push]` in the daemon's config: all empty, and push off, by default.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PushConfig {
    pub topic: String,
    pub key_id: String,
    pub team_id: String,
    pub key_path: PathBuf,
    pub sandbox: bool,
}

impl PushConfig {
    /// Whether every part push needs is set. Anything less is off, never a
    /// half-configured push that fails at the first notice.
    pub fn configured(&self) -> bool {
        !self.topic.trim().is_empty()
            && !self.key_id.trim().is_empty()
            && !self.team_id.trim().is_empty()
            && !self.key_path.as_os_str().is_empty()
    }

    /// One line for the startup log.
    pub fn status(&self) -> &'static str {
        if self.configured() {
            "phone push is configured, but this build doesn't send it yet"
        } else {
            "phone push is off: [push] isn't configured"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_is_off_until_every_part_is_set() {
        let mut cfg = PushConfig::default();
        assert!(!cfg.configured());
        cfg.topic = "com.example.app".into();
        cfg.key_id = "KEY".into();
        cfg.team_id = "TEAM".into();
        assert!(!cfg.configured(), "no key file");
        cfg.key_path = PathBuf::from("/keys/push.p8");
        assert!(cfg.configured());
        cfg.team_id = "  ".into();
        assert!(!cfg.configured());
    }

    #[test]
    fn a_config_without_push_reads_as_off() {
        let cfg: PushConfig = toml::from_str("").expect("empty");
        assert_eq!(cfg, PushConfig::default());
        let cfg: PushConfig = toml::from_str("topic = \"t\"\nsandbox = true").expect("partial");
        assert!(cfg.sandbox && !cfg.configured());
    }
}
