//! One Cargo target per bot (H-029). Each worktree building its own
//! `target/` costs 7–27 GB, so every bot session gets `CARGO_TARGET_DIR`
//! pointing at `<bot dir>/cargo-target`, beside its workspace. Two worktrees
//! of the same bot share it; Cargo's own lock serialises their builds.
//!
//! - Workers on Windows keep the machine's shared target (H-109).
//! - A `CARGO_TARGET_DIR` the bot sets itself, in its settings' `env`, wins:
//!   Claude Code applies those over the session's environment.
//! - The folder is inside the bot's own, so its guard lets it `cargo clean`
//!   or remove it, and no other bot's guard does.
//! - Deleting the bot removes it: it is build output, not work.
//!
//! Lighter checkouts (0.23.0) add the computer's shared caches beside it:
//! Rust builds go through one `sccache` under the home when it is installed
//! (nothing changes when it isn't), and pnpm installs share one content
//! store there. A variable the session already has wins, as above.

use std::path::{Path, PathBuf};

/// The bot's own target folder, a sibling of its workspace.
pub const BOT_TARGET_DIR: &str = "cargo-target";
/// The variable Cargo reads.
pub const CARGO_TARGET_ENV: &str = "CARGO_TARGET_DIR";

/// `<bot dir>/cargo-target` for a bot whose workspace is `workspace`.
pub fn bot_target(workspace: &Path) -> PathBuf {
    workspace.parent().unwrap_or(workspace).join(BOT_TARGET_DIR)
}

/// Adds the bot's own target unless the session already has one (a
/// Windows worker's shared target).
pub fn with_bot_target(env: &mut Vec<(String, String)>, workspace: &Path) {
    if !workspace.is_absolute() || env.iter().any(|(key, _)| key == CARGO_TARGET_ENV) {
        return;
    }
    env.push((
        CARGO_TARGET_ENV.to_string(),
        bot_target(workspace).display().to_string(),
    ));
}

/// Rust's shared compiler cache, under `<home>/cache`.
pub const SCCACHE_DIR: &str = "sccache";
/// pnpm's shared content store, under `<home>/cache`.
pub const PNPM_STORE_DIR: &str = "pnpm-store";

/// Adds the computer's shared build caches: `sccache` as Cargo's
/// `RUSTC_WRAPPER` when it is on the tools' `PATH`, and pnpm's store.
pub fn with_shared_caches(env: &mut Vec<(String, String)>, home: &Path) {
    let sccache = crate::tool_path::which("sccache", &crate::tool_path::tool_path());
    with_caches(env, home, sccache.as_deref());
}

/// [`with_shared_caches`] with `sccache` found (or not) already.
pub fn with_caches(env: &mut Vec<(String, String)>, home: &Path, sccache: Option<&Path>) {
    let cache = home.join("cache");
    let mut set = |key: &str, value: PathBuf| {
        if !env.iter().any(|(k, _)| k == key) {
            env.push((key.to_string(), value.display().to_string()));
        }
    };
    if let Some(sccache) = sccache {
        set("RUSTC_WRAPPER", sccache.to_path_buf());
        set("SCCACHE_DIR", cache.join(SCCACHE_DIR));
    }
    set("npm_config_store_dir", cache.join(PNPM_STORE_DIR));
}

/// Removes a deleted bot's target folder in the background: it can be tens
/// of GB, and nothing waits on it.
pub fn remove_bot_target(workspace: &Path) {
    // A linked bot runs elsewhere and has no workspace here: nothing of ours
    // to remove, and never a path relative to the daemon's own directory.
    if !workspace.is_absolute() {
        return;
    }
    let target = bot_target(workspace);
    if !target.exists() {
        return;
    }
    std::thread::spawn(move || match std::fs::remove_dir_all(&target) {
        Ok(()) => tracing::info!(path = %target.display(), "removed a deleted bot's cargo target"),
        Err(error) => {
            tracing::warn!(path = %target.display(), %error, "can't remove a deleted bot's cargo target");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bot_builds_into_its_own_folder_unless_it_has_one() {
        let dir = tempfile::tempdir().unwrap();
        let bot = dir
            .path()
            .join("projects")
            .join("p")
            .join("bots")
            .join("dev");
        let workspace = bot.join("workspace");
        // A linked bot has no workspace here.
        let mut linked = Vec::new();
        with_bot_target(&mut linked, Path::new(""));
        assert!(linked.is_empty());
        let mut env = vec![("FLOTILLA_TOKEN".to_string(), "t".to_string())];
        with_bot_target(&mut env, &workspace);
        let expected = bot.join(BOT_TARGET_DIR).display().to_string();
        assert_eq!(
            env.iter()
                .find(|(k, _)| k == CARGO_TARGET_ENV)
                .map(|(_, v)| v),
            Some(&expected)
        );
        let mut shared = vec![(CARGO_TARGET_ENV.to_string(), "shared".to_string())];
        with_bot_target(&mut shared, &workspace);
        assert_eq!(
            shared,
            [(CARGO_TARGET_ENV.to_string(), "shared".to_string())]
        );
    }

    #[test]
    fn shared_caches_follow_sccache_and_never_replace_a_bots_own() {
        let home = Path::new("/h");
        let get = |env: &[(String, String)], key: &str| {
            env.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone())
        };
        let mut without = Vec::new();
        with_caches(&mut without, home, None);
        assert_eq!(
            get(&without, "RUSTC_WRAPPER"),
            None,
            "no sccache, no wrapper"
        );
        assert_eq!(get(&without, "SCCACHE_DIR"), None);
        let store = home
            .join("cache")
            .join(PNPM_STORE_DIR)
            .display()
            .to_string();
        assert_eq!(get(&without, "npm_config_store_dir"), Some(store.clone()));

        let bin = Path::new("/opt/bin/sccache");
        let mut with = Vec::new();
        with_caches(&mut with, home, Some(bin));
        assert_eq!(get(&with, "RUSTC_WRAPPER"), Some(bin.display().to_string()));
        let dir = home.join("cache").join(SCCACHE_DIR).display().to_string();
        assert_eq!(get(&with, "SCCACHE_DIR"), Some(dir));
        assert_eq!(get(&with, "npm_config_store_dir"), Some(store));

        let mut own = vec![("RUSTC_WRAPPER".to_string(), "mine".to_string())];
        with_caches(&mut own, home, Some(bin));
        assert_eq!(get(&own, "RUSTC_WRAPPER"), Some("mine".to_string()));
        assert_eq!(own.iter().filter(|(k, _)| k == "RUSTC_WRAPPER").count(), 1);
    }

    #[test]
    fn a_deleted_bots_target_goes() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("dev").join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        let target = bot_target(&workspace);
        std::fs::create_dir_all(target.join("debug")).unwrap();
        std::fs::write(target.join("debug").join("flotillad"), b"x").unwrap();
        remove_bot_target(&workspace);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while target.exists() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(!target.exists());
        assert!(workspace.exists(), "the workspace is kept");
    }
}
