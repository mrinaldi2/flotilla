//! Every name the daemon shows, registers or reads, in one place.
//!
//! The project is Flotilla. It was The Hermes before, and Gravity before
//! that. Names that live on disk, in the OS or in running bots' history keep
//! working under every earlier spelling: those are the `LEGACY_*` lists
//! below, newest first, and nothing else in the daemon spells them out.

use std::ffi::OsString;

/// The product name, for titles and anything the owner reads once.
pub const DISPLAY_NAME: &str = "Flotilla";

/// The name in running copy ("watch it from Flotilla", "[Flotilla] restarting").
pub const SHORT_NAME: &str = "Flotilla";

/// The product's names before this one, newest first. Old transcripts and
/// bot memories still use them.
pub const LEGACY_DISPLAY_NAMES: [&str; 2] = ["The Hermes", "Gravity"];

/// Machine-facing identifier, e.g. the Codex client name.
pub const SLUG: &str = "flotilla";

/// Daemon environment variables are read under this prefix first.
pub const ENV_PREFIX: &str = "FLOTILLA_";

/// Read, in this order, when the `ENV_PREFIX` variable is unset, so existing
/// setups keep working.
pub const LEGACY_ENV_PREFIXES: [&str; 2] = ["THEHERMES_", "GRAVITY_"];

/// The bus's MCP server name.
pub const MCP_SERVER: &str = "flotilla-bus";

/// The bus's MCP server names before the renames. Transcripts written under
/// them stay readable forever.
pub const LEGACY_MCP_SERVERS: [&str; 2] = ["hermes-bus", "gravity-bus"];

/// The name bots are registered under: `mcp.json`, Codex's config, the MCP
/// `serverInfo` and the system prompt all use it.
pub const ACTIVE_MCP_SERVER: &str = MCP_SERVER;

/// Tool-name prefixes of the bus in transcripts, newest first. Sessions from
/// before a rename keep `mcp__hermes-bus__*` or `mcp__gravity-bus__*` calls
/// forever, so readers accept all three.
pub const BUS_TOOL_PREFIXES: [&str; 3] = [
    "mcp__flotilla-bus__",
    "mcp__hermes-bus__",
    "mcp__gravity-bus__",
];

/// The bus tool a transcript's tool name refers to, under any name.
pub fn bus_tool(name: &str) -> Option<&str> {
    BUS_TOOL_PREFIXES
        .iter()
        .find_map(|prefix| name.strip_prefix(prefix))
}

/// Prefix of the daemon's JSON-RPC methods on the local endpoint
/// (`flotilla/owner_ticket`, `flotilla/pr_merge`, …).
pub const RPC_PREFIX: &str = "flotilla/";

/// The prefix before the rename. A CLI, hook or app from before it still
/// sends `hermes/<method>`, so the daemon answers both.
pub const LEGACY_RPC_PREFIX: &str = "hermes/";

/// `flotilla/<name>`.
pub fn rpc_method(name: &str) -> String {
    format!("{RPC_PREFIX}{name}")
}

/// The bare name of a method sent under either prefix: `pr_merge` for
/// `flotilla/pr_merge` and `hermes/pr_merge`.
pub fn rpc_name(method: &str) -> Option<&str> {
    method
        .strip_prefix(RPC_PREFIX)
        .or_else(|| method.strip_prefix(LEGACY_RPC_PREFIX))
}

/// The daemon home under the user's home directory.
pub const HOME_DIR_NAME: &str = ".flotilla";

/// The homes before the renames, newest first. `flotillad migrate-home`
/// moves the newest one that holds a daemon to [`HOME_DIR_NAME`] and leaves
/// a link behind for one release.
pub const LEGACY_HOME_DIR_NAMES: [&str; 2] = [".thehermes", ".gravity"];

/// Stem of the daemon's own files in its home: `flotillad.toml`, `.port`,
/// `.lock`, `.reclaims`, `logs/flotillad.{out,err}.log`, `flotillad-task.*`.
/// It is also the daemon binary's name.
pub const DAEMON_FILE_STEM: &str = "flotillad";

/// The same files (and binaries) before the renames, newest first; the home
/// migration renames them.
pub const LEGACY_DAEMON_FILE_STEMS: [&str; 2] = ["hermesd", "gravityd"];

/// The launchd agent label on macOS.
pub const LAUNCHD_LABEL: &str = "com.manuelrinaldi.flotillad";

/// The labels releases before the renames installed under, newest first.
/// `service install` unloads them before loading [`LAUNCHD_LABEL`], so two
/// never run at once.
pub const LEGACY_LAUNCHD_LABELS: [&str; 2] =
    ["com.manuelrinaldi.thehermesd", "in.mikolajczuk.gravityd"];

/// Prefix of the per-user scheduled task on Windows.
pub const WINDOWS_TASK: &str = "Flotilla";

/// The task prefixes before the renames, removed by `service install`.
pub const LEGACY_WINDOWS_TASKS: [&str; 2] = ["The Hermes", "Gravity"];

/// `<stem><suffix>` in the daemon home, e.g. `daemon_file(".toml")`.
pub fn daemon_file(suffix: &str) -> String {
    format!("{DAEMON_FILE_STEM}{suffix}")
}

/// The pre-rename names of [`daemon_file`], newest first.
pub fn legacy_daemon_files(suffix: &str) -> Vec<String> {
    LEGACY_DAEMON_FILE_STEMS
        .iter()
        .map(|stem| format!("{stem}{suffix}"))
        .collect()
}

/// [`daemon_file`] followed by every [`legacy_daemon_files`] name.
pub fn all_daemon_files(suffix: &str) -> Vec<String> {
    std::iter::once(daemon_file(suffix))
        .chain(legacy_daemon_files(suffix))
        .collect()
}

/// Prefix of on-disk markers and branch names written into bots' repos and
/// workspaces (`.flotilla-worktree.json`, `flotilla/<bot>`).
pub const ON_DISK_SLUG: &str = "flotilla";

/// The prefix before the rename (The Hermes kept Gravity's). Worktrees and
/// branches already in bots' repositories live outside the home, so the
/// migration leaves them, and readers recognise both.
pub const LEGACY_ON_DISK_SLUG: &str = "gravity";

/// The variable a bot's session reads its bus token from. Bots also get the
/// [`LEGACY_BOT_TOKEN_ENVS`], so a config written before the rename still
/// authenticates until it is regenerated.
pub const BOT_TOKEN_ENV: &str = "FLOTILLA_TOKEN";
pub const LEGACY_BOT_TOKEN_ENVS: [&str; 2] = ["THEHERMES_TOKEN", "GRAVITY_TOKEN"];

/// The token variables a bot session starts with: the current name and the
/// legacy ones, carrying the same token.
pub fn bot_token_vars(token: String) -> Vec<(String, String)> {
    std::iter::once(BOT_TOKEN_ENV)
        .chain(LEGACY_BOT_TOKEN_ENVS)
        .map(|name| (name.to_string(), token.clone()))
        .collect()
}

/// Author of the commits a worker saves, and its non-routable address.
pub const WORKER_GIT_SUFFIX: &str = "(Flotilla worker)";
pub const WORKER_GIT_EMAIL: &str = "worker@flotilla.invalid";

/// `FLOTILLA_<name>`.
pub fn env_name(name: &str) -> String {
    format!("{ENV_PREFIX}{name}")
}

/// `THEHERMES_<name>` and `GRAVITY_<name>`, still honoured and still set
/// for bots.
pub fn legacy_env_names(name: &str) -> Vec<String> {
    LEGACY_ENV_PREFIXES
        .iter()
        .map(|prefix| format!("{prefix}{name}"))
        .collect()
}

/// [`env_name`] followed by every [`legacy_env_names`] name.
pub fn all_env_names(name: &str) -> Vec<String> {
    std::iter::once(env_name(name))
        .chain(legacy_env_names(name))
        .collect()
}

/// Reads `FLOTILLA_<name>`, falling back to `THEHERMES_<name>`, then
/// `GRAVITY_<name>`.
pub fn env_var_os(name: &str) -> Option<OsString> {
    read_env(name, |key| std::env::var_os(key))
}

fn read_env(name: &str, get: impl Fn(&str) -> Option<OsString>) -> Option<OsString> {
    all_env_names(name).iter().find_map(|key| get(key))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> {
        let map: HashMap<String, OsString> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), OsString::from(v)))
            .collect();
        move |key| map.get(key).cloned()
    }

    #[test]
    fn the_new_prefix_wins_over_the_old_ones() {
        let get = env(&[
            ("FLOTILLA_HOME", "/new"),
            ("THEHERMES_HOME", "/hermes"),
            ("GRAVITY_HOME", "/old"),
        ]);
        assert_eq!(read_env("HOME", get), Some(OsString::from("/new")));
        let get = env(&[("THEHERMES_HOME", "/hermes"), ("GRAVITY_HOME", "/old")]);
        assert_eq!(read_env("HOME", get), Some(OsString::from("/hermes")));
    }

    #[test]
    fn the_old_prefixes_still_work_alone() {
        let get = env(&[("GRAVITY_HOME", "/old")]);
        assert_eq!(read_env("HOME", get), Some(OsString::from("/old")));
        let get = env(&[("THEHERMES_HOME", "/hermes")]);
        assert_eq!(read_env("HOME", get), Some(OsString::from("/hermes")));
        assert_eq!(read_env("HOME", env(&[])), None);
    }

    #[test]
    fn bus_tools_are_read_under_every_name() {
        for prefix in BUS_TOOL_PREFIXES {
            assert_eq!(
                bus_tool(&format!("{prefix}send_message")),
                Some("send_message")
            );
        }
        assert_eq!(bus_tool("mcp__playwright__browser_click"), None);
        for (prefix, server) in BUS_TOOL_PREFIXES
            .iter()
            .zip(std::iter::once(MCP_SERVER).chain(LEGACY_MCP_SERVERS))
        {
            assert_eq!(*prefix, format!("mcp__{server}__"));
        }
    }

    #[test]
    fn rpc_methods_are_answered_under_both_prefixes() {
        assert_eq!(rpc_method("pr_merge"), "flotilla/pr_merge");
        assert_eq!(rpc_name("flotilla/pr_merge"), Some("pr_merge"));
        assert_eq!(rpc_name("hermes/pr_merge"), Some("pr_merge"));
        assert_eq!(rpc_name("gravity/pr_merge"), None);
        assert_eq!(rpc_name("pr_merge"), None);
    }

    #[test]
    fn names_follow_the_prefixes() {
        assert_eq!(env_name("TOKEN"), "FLOTILLA_TOKEN");
        assert_eq!(
            legacy_env_names("TOKEN"),
            ["THEHERMES_TOKEN", "GRAVITY_TOKEN"]
        );
        assert_eq!(BOT_TOKEN_ENV, env_name("TOKEN"));
        assert_eq!(LEGACY_BOT_TOKEN_ENVS.to_vec(), legacy_env_names("TOKEN"));
        let vars = bot_token_vars("t".into());
        assert_eq!(vars.len(), 3);
        assert!(vars.iter().all(|(_, token)| token == "t"));
        assert_eq!(
            all_daemon_files(".toml"),
            ["flotillad.toml", "hermesd.toml", "gravityd.toml"]
        );
    }
}
