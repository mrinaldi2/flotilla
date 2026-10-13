//! The tools each computer has (H-283, H-261 §1.6). Every daemon probes
//! them at boot and hourly, keeps its own in `machine_tool` under its name,
//! and sends them to each linked computer, which keeps them under the name
//! it knows this one by. Check routing reads them (§7).

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::app::AppState;
use crate::board::release::machines;
use crate::prs::check_route::{DISK_FREE_GB, OS};

/// The tools probed, and the flag that prints each one's version.
pub const TOOLS: [(&str, &str); 7] = [
    ("node", "--version"),
    ("pnpm", "--version"),
    ("cargo", "--version"),
    ("xcodebuild", "-version"),
    ("python3", "--version"),
    ("mkdocs", "--version"),
    ("docker", "--version"),
];

/// The peer event that carries a computer's tools.
pub const FRAME: &str = "machine_tools";

/// The longest one tool may take to say its version.
const TOOL_TIMEOUT: Duration = Duration::from_secs(15);

/// The tools found on the checks' `PATH` ([`tool_path`], worked out again
/// now) with their versions, plus its OS and the free disk where `home` is.
///
/// [`tool_path`]: crate::tool_path::tool_path
pub fn probe(home: &Path) -> BTreeMap<String, String> {
    probe_on(home, &crate::tool_path::refresh())
}

/// [`probe`] with `path` as the `PATH` the tools are found on and run with.
pub fn probe_on(home: &Path, path: &OsStr) -> BTreeMap<String, String> {
    let mut found: BTreeMap<String, String> = TOOLS
        .iter()
        .filter_map(|(tool, flag)| Some((tool.to_string(), version_of(tool, flag, path)?)))
        .collect();
    found.insert(OS.to_string(), std::env::consts::OS.to_string());
    if let Some(free) = crate::migrate_home::disk::free_bytes(home) {
        found.insert(DISK_FREE_GB.to_string(), (free / 1_000_000_000).to_string());
    }
    found
}

/// `tool flag`'s version, or `None` when it isn't on `path` or doesn't
/// answer. The tool runs with `path` as its `PATH`, as a check would.
fn version_of(tool: &str, flag: &str, path: &OsStr) -> Option<String> {
    let program = crate::tool_path::which(tool, path)?;
    let mut child = Command::new(program)
        .arg(flag)
        .env("PATH", path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;
    let started = Instant::now();
    loop {
        if child.try_wait().ok()?.is_some() {
            break;
        }
        if started.elapsed() > TOOL_TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let out = child.wait_with_output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    Some(parse_version(&text).unwrap_or_else(|| "unknown".to_string()))
}

/// The first version-looking word: `v24.1.0` → `24.1.0`, `Python 3.12.1`
/// → `3.12.1`, `Docker version 27.1.1, build …` → `27.1.1`.
pub fn parse_version(text: &str) -> Option<String> {
    text.split(|c: char| c.is_whitespace() || c == ',')
        .map(|w| w.strip_prefix('v').unwrap_or(w))
        .find(|w| w.chars().next().is_some_and(|c| c.is_ascii_digit()) && w.contains('.'))
        .map(str::to_string)
}

/// Probes now, keeps the result as this computer's and sends it to every
/// linked computer that is online.
pub async fn probe_and_share(app: &Arc<AppState>) -> anyhow::Result<()> {
    let home = app.cfg.home.clone();
    let tools = tokio::task::spawn_blocking(move || probe(&home)).await?;
    app.db.board_tx(|t| {
        let here = machines::this_computer(t)?;
        t.set_machine_tools(&here, &tools)
    })?;
    for peer in app.db.list_peers()? {
        if peer.revoked_at.is_none() && app.peers.is_online(&peer.id) {
            app.peers.notify(&peer.id, frame(&tools));
        }
    }
    crate::prs::check_jobs::nudge();
    Ok(())
}

fn frame(tools: &BTreeMap<String, String>) -> Value {
    json!({ "type": FRAME, "tools": tools })
}

/// A newly linked computer gets this one's last probe at once, rather than
/// at the next hourly one.
pub fn link_up(app: &AppState, peer_id: &str) {
    let tools = app.db.board_read(|t| {
        let here = machines::this_computer(t)?;
        t.machine_tools(&here)
    });
    if let Some(tools) = tools.ok().filter(|t| !t.is_empty()) {
        app.peers.notify(peer_id, frame(&tools));
    }
}

/// A linked computer's tools, kept under the name this one knows it by.
pub fn receive(app: &AppState, peer_id: &str, frame: &Value) {
    let Ok(Some(peer)) = app.db.get_peer(peer_id) else {
        return;
    };
    let tools: BTreeMap<String, String> = frame
        .get("tools")
        .and_then(Value::as_object)
        .map(|o| {
            o.iter()
                .filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_string())))
                .filter(|(k, v)| k.len() <= 64 && v.len() <= 128)
                .take(64)
                .collect()
        })
        .unwrap_or_default();
    match app.db.board_tx(|t| t.set_machine_tools(&peer.name, &tools)) {
        Ok(()) => crate::prs::check_jobs::nudge(),
        Err(error) => tracing::warn!(peer = %peer.name, %error, "machine tools not kept"),
    }
}

/// Probes at boot, then every `probe_interval_secs` (0: never).
pub fn spawn(app: Arc<AppState>) {
    if app.cfg.checks.probe_interval_secs == 0 {
        return;
    }
    let interval = Duration::from_secs(app.cfg.checks.probe_interval_secs.max(60));
    tokio::spawn(async move {
        loop {
            if let Err(error) = probe_and_share(&app).await {
                tracing::warn!(%error, "probing this computer's tools failed");
            }
            tokio::time::sleep(interval).await;
        }
    });
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use std::path::Path;

    use super::parse_version;

    #[test]
    fn versions_are_read_from_each_tools_own_words() {
        let cases = [
            ("v24.1.0\n", "24.1.0"),
            ("10.12.1\n", "10.12.1"),
            ("cargo 1.90.0 (840b83a10 2025-07-30)\n", "1.90.0"),
            ("Xcode 16.0\nBuild version 16A242d\n", "16.0"),
            ("Python 3.12.1\n", "3.12.1"),
            ("mkdocs, version 1.6.0 from /x (Python 3.12)\n", "1.6.0"),
            ("Docker version 27.1.1, build 6312585\n", "27.1.1"),
        ];
        for (text, want) in cases {
            assert_eq!(parse_version(text).as_deref(), Some(want), "{text}");
        }
        assert_eq!(parse_version("no version here"), None);
    }

    #[test]
    fn a_probe_names_the_os_and_skips_what_isnt_there() {
        let dir = tempfile::tempdir().unwrap();
        let tools = super::probe(dir.path());
        assert!(tools.contains_key(super::OS));
        assert!(tools.contains_key(super::DISK_FREE_GB));
        assert!(tools.keys().all(|k| k == super::OS
            || k == super::DISK_FREE_GB
            || super::TOOLS.iter().any(|(t, _)| t == k)));
    }

    /// H-318: cargo only in the rustup toolchain under HOME, a bare PATH,
    /// and the probe still finds it through the derived PATH.
    #[cfg(unix)]
    #[test]
    fn cargo_in_the_rustup_toolchain_is_found_with_a_bare_path() {
        use std::os::unix::fs::PermissionsExt;
        let home = tempfile::tempdir().unwrap();
        let rustup = home.path().join(".rustup");
        let bin = rustup.join("toolchains").join("stable-x").join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(
            rustup.join("settings.toml"),
            "default_toolchain = \"stable-x\"\n",
        )
        .unwrap();
        let cargo = bin.join("cargo");
        std::fs::write(&cargo, "#!/bin/sh\necho 'cargo 1.97.0 (fake 2026-01-01)'\n").unwrap();
        std::fs::set_permissions(&cargo, std::fs::Permissions::from_mode(0o755)).unwrap();

        let env = |k: &str| (k == "PATH").then(|| "/usr/bin:/bin".into());
        // Only folders under the fake HOME: this computer's own rustup and
        // Homebrew stay out of it.
        let is_dir = |p: &Path| {
            p.starts_with(home.path()) && std::fs::symlink_metadata(p).is_ok_and(|m| m.is_dir())
        };
        let read = |p: &Path| std::fs::read_to_string(p).ok();
        let host = crate::tool_path::Host {
            env: &env,
            home: Some(home.path().to_path_buf()),
            is_dir: &is_dir,
            read: &read,
        };
        let path = crate::tool_path::derive(&host);
        let bare = super::probe_on(home.path(), std::ffi::OsStr::new("/usr/bin:/bin"));
        assert!(!bare.contains_key("cargo") || which_cargo_on_bare_path());
        let tools = super::probe_on(home.path(), &path);
        assert_eq!(tools.get("cargo").map(String::as_str), Some("1.97.0"));
    }

    /// The test computer itself may have cargo in /usr/bin.
    #[cfg(unix)]
    fn which_cargo_on_bare_path() -> bool {
        crate::tool_path::which("cargo", std::ffi::OsStr::new("/usr/bin:/bin")).is_some()
    }
}
