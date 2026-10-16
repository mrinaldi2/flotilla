//! The one `PATH` checks see (H-318): the daemon's own, then the per-user
//! tool folders that exist here and aren't on it yet. launchd starts the
//! daemon with a bare `PATH`, so Homebrew's keg-only rustup proxies, the
//! rustup toolchain and pnpm would otherwise be missing. The probe
//! ([`crate::machine_tools`]) and the runner ([`crate::check_exec`]) both
//! use it, so a check is never routed to a computer whose run then can't
//! find its tool. No login shell and no dotfile runs, unlike
//! `owner_action::login_path`.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::sync::RwLock;

fn cached() -> &'static RwLock<Option<OsString>> {
    static PATH: RwLock<Option<OsString>> = RwLock::new(None);
    &PATH
}

/// The derived `PATH`, worked out once and again at each [`refresh`].
pub fn tool_path() -> OsString {
    if let Some(path) = cached().read().unwrap_or_else(|e| e.into_inner()).clone() {
        return path;
    }
    refresh()
}

/// Works the `PATH` out again (each probe): a tool installed since shows.
pub fn refresh() -> OsString {
    let path = derive(&Host::real());
    *cached().write().unwrap_or_else(|e| e.into_inner()) = Some(path.clone());
    path
}

/// What [`derive`] reads of the computer, so tests can fake it.
pub struct Host<'a> {
    pub env: &'a dyn Fn(&str) -> Option<OsString>,
    pub home: Option<PathBuf>,
    /// A real folder: there, and not itself a symlink.
    pub is_dir: &'a dyn Fn(&Path) -> bool,
    pub read: &'a dyn Fn(&Path) -> Option<String>,
}

impl Host<'static> {
    fn real() -> Self {
        Host {
            env: &|k| std::env::var_os(k),
            home: home_dir(),
            is_dir: &real_dir,
            read: &|p| std::fs::read_to_string(p).ok(),
        }
    }
}

fn home_dir() -> Option<PathBuf> {
    let key = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(key)
        .filter(|h| !h.is_empty())
        .map(PathBuf::from)
}

fn real_dir(p: &Path) -> bool {
    std::fs::symlink_metadata(p).is_ok_and(|m| m.is_dir())
}

/// The daemon's `PATH` as it is, then each candidate that is a real folder
/// and isn't on it already.
pub fn derive(host: &Host<'_>) -> OsString {
    let mut dirs: Vec<PathBuf> = (host.env)("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    for dir in candidates(host) {
        if !dirs.contains(&dir) && (host.is_dir)(&dir) {
            dirs.push(dir);
        }
    }
    std::env::join_paths(dirs).unwrap_or_default()
}

/// Where per-user tools live, most specific first: the rustup proxies
/// before the toolchain, as they honour a repository's `rust-toolchain`.
#[cfg(not(windows))]
fn candidates(host: &Host<'_>) -> Vec<PathBuf> {
    let env = |k: &str| (host.env)(k).filter(|v| !v.is_empty()).map(PathBuf::from);
    let home = |rel: &str| host.home.as_ref().map(|h| h.join(rel));
    [
        env("CARGO_HOME").map_or_else(|| home(".cargo/bin"), |c| Some(c.join("bin"))),
        Some("/opt/homebrew/opt/rustup/bin".into()),
        Some("/usr/local/opt/rustup/bin".into()),
        rustup_toolchain(host, env("RUSTUP_HOME").or_else(|| home(".rustup"))),
        env("PNPM_HOME"),
        home("Library/pnpm"),
        home(".local/share/pnpm"),
        home(".volta/bin"),
        home(".local/bin"),
        Some("/opt/homebrew/bin".into()),
        Some("/usr/local/bin".into()),
    ]
    .into_iter()
    .flatten()
    .collect()
}

#[cfg(windows)]
fn candidates(host: &Host<'_>) -> Vec<PathBuf> {
    let env = |k: &str| (host.env)(k).filter(|v| !v.is_empty()).map(PathBuf::from);
    [
        env("CARGO_HOME")
            .map(|c| c.join("bin"))
            .or_else(|| host.home.as_ref().map(|h| h.join(".cargo").join("bin"))),
        rustup_toolchain(
            host,
            env("RUSTUP_HOME").or_else(|| host.home.as_ref().map(|h| h.join(".rustup"))),
        ),
        env("PNPM_HOME"),
        env("LOCALAPPDATA").map(|d| d.join("pnpm")),
        env("APPDATA").map(|d| d.join("npm")),
        env("ProgramFiles").map(|d| d.join("nodejs")),
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// `<rustup home>/toolchains/<default>/bin`, the default read from its
/// `settings.toml`; a short name (`stable`) gets this computer's triple.
fn rustup_toolchain(host: &Host<'_>, rustup: Option<PathBuf>) -> Option<PathBuf> {
    let rustup = rustup?;
    let name = default_toolchain(&(host.read)(&rustup.join("settings.toml"))?)?;
    let toolchains = rustup.join("toolchains");
    let exact = toolchains.join(&name).join("bin");
    if (host.is_dir)(&exact) {
        return Some(exact);
    }
    Some(toolchains.join(format!("{name}-{}", triple())).join("bin"))
}

/// The `default_toolchain = "…"` line of rustup's `settings.toml`.
pub fn default_toolchain(settings: &str) -> Option<String> {
    settings.lines().find_map(|line| {
        let (key, value) = line.split_once('=')?;
        let value = value.trim().trim_matches('"');
        (key.trim() == "default_toolchain"
            && !value.is_empty()
            && !value.contains(['/', '\\'])
            && !value.contains(".."))
        .then(|| value.to_string())
    })
}

fn triple() -> String {
    let (arch, os) = (std::env::consts::ARCH, std::env::consts::OS);
    match os {
        "macos" => format!("{arch}-apple-darwin"),
        "windows" => format!("{arch}-pc-windows-msvc"),
        _ => format!("{arch}-unknown-linux-gnu"),
    }
}

/// `tool` resolved against `path`, as the runner's shell would: `Command::new`
/// looks a program up on the daemon's own `PATH`, not the child's. Folder by
/// folder; on Windows each folder tries the [`pathext`] extensions in order,
/// as `cmd /C` does (the runner keeps `PATHEXT`), elsewhere the file must be
/// executable, as `sh` requires.
pub fn which(tool: &str, path: &OsStr) -> Option<PathBuf> {
    let names: Vec<String> = if cfg!(windows) {
        pathext(std::env::var_os("PATHEXT").as_deref())
            .iter()
            .map(|e| format!("{tool}{e}"))
            .collect()
    } else {
        vec![tool.to_string()]
    };
    std::env::split_paths(path)
        .flat_map(|dir| names.iter().map(move |n| dir.join(n)))
        .find(|p| runnable(p))
}

/// The extensions `cmd` tries, lower-cased and in order: `PATHEXT`, or cmd's
/// own default when it is unset or empty.
pub fn pathext(pathext: Option<&OsStr>) -> Vec<String> {
    let list = pathext
        .map(|p| p.to_string_lossy().into_owned())
        .filter(|p| !p.trim().is_empty())
        .unwrap_or_else(|| ".COM;.EXE;.BAT;.CMD".into());
    list.split(';')
        .map(str::trim)
        .filter(|e| e.len() > 1 && e.starts_with('.'))
        .map(str::to_ascii_lowercase)
        .collect()
}

#[cfg(unix)]
fn runnable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(p).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn runnable(p: &Path) -> bool {
    p.is_file()
}

#[cfg(test)]
#[path = "tool_path_tests.rs"]
mod tests;
