//! The daemon's GitHub token for [`super::SafeGit::fetching`] (H-289,
//! H-321): read from its environment or gh, given only to github.com over
//! https, through an owner-only file a credential helper reads.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use super::run_dir;

/// Writes `token` to a new owner-only file under [`run_dir`].
pub(super) fn write_token(token: &str) -> anyhow::Result<PathBuf> {
    let file = run_dir()?.join(format!(
        "token-{}-{:016x}",
        std::process::id(),
        rand::random::<u64>()
    ));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(&file)?.write_all(token.as_bytes())?;
    Ok(file)
}

/// `s` as one single-quoted shell word.
pub(super) fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// The only place the token goes; the helper is scoped to it.
pub(super) const GITHUB: &str = "https://github.com";

/// Whether `url` is an https URL on github.com itself (any user part, no
/// other host, port or look-alike such as `github.com.example`).
pub(super) fn is_github(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let host = authority.rsplit('@').next().unwrap_or_default();
    host.eq_ignore_ascii_case("github.com")
}

/// The daemon's GitHub token for a fetch from `url`: none unless `url` is
/// on github.com, and none on a computer without one (git then fetches
/// anonymously, which a public repository allows).
pub(crate) fn daemon_token(url: &str) -> Option<String> {
    is_github(url).then(github_token).flatten()
}

/// The daemon's GitHub token: from its environment, else gh's (asked
/// outside any repository).
fn github_token() -> Option<String> {
    for key in ["GH_TOKEN", "GITHUB_TOKEN"] {
        if let Some(t) = std::env::var(key).ok().filter(|t| !t.is_empty()) {
            return Some(t);
        }
    }
    let out = Command::new("gh")
        .args(["auth", "token", "--hostname", "github.com"])
        .current_dir(std::env::temp_dir())
        .stdin(Stdio::null())
        .output()
        .ok()?;
    let token = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (out.status.success() && !token.is_empty()).then_some(token)
}
