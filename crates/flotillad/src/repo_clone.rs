//! Cloning a project's repository into a folder the owner picks (0.20.0,
//! `project_repo_clone`), from the phone's project setup. The owner's alone:
//! no bot tool reaches it. The folder must be inside the daemon user's home
//! (symlinks followed), not the home itself nor the daemon's own folder,
//! and new or empty. The clone runs through [`SafeGit`] (no hooks, no
//! credential prompts, https and local paths only), one per project at a
//! time, and every client hears how it went: `repo_clone {project_id,
//! state}`, `cloning`, then `done` with the path or `failed` with a
//! sentence. A failed clone leaves nothing behind.

use std::collections::HashSet;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::app::AppState;
use crate::events::Push;
use crate::safe_git::{self, SafeGit};

/// A transfer slower than 1 KB/s for a minute is given up, so a clone
/// never hangs the project's slot.
const LOW_SPEED: [&str; 2] = ["http.lowSpeedLimit=1024", "http.lowSpeedTime=60"];

/// The projects with a clone running.
#[derive(Default)]
pub struct Clones {
    running: Mutex<HashSet<String>>,
}

impl Clones {
    /// Claims the project's slot; `false` while a clone for it runs.
    pub fn begin(&self, project_id: &str) -> bool {
        self.lock().insert(project_id.to_string())
    }

    fn finish(&self, project_id: &str) {
        self.lock().remove(project_id);
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashSet<String>> {
        self.running.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Where a clone goes, resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dest {
    /// The clone's folder, symlinks resolved.
    pub path: PathBuf,
    /// The nearest folder above it that exists: git runs there, and a
    /// failed clone removes what it made below it.
    base: PathBuf,
    /// The folder existed (empty) before the clone.
    existed: bool,
}

/// The daemon clones over https or from a folder on this computer: its git
/// has no SSH keys, and plain http would send a token in the clear.
pub fn check_url(url: &str) -> Result<(), String> {
    let lower = url.to_ascii_lowercase();
    if lower.starts_with("https://") || lower.starts_with("file://") || Path::new(url).is_absolute()
    {
        return Ok(());
    }
    Err(format!(
        "Clone '{url}' over https, or from a folder on this computer: the daemon's git \
         has no SSH keys."
    ))
}

/// `raw` (absolute, or `~/…` in `user_home`) as a folder to clone into.
pub fn destination(user_home: &Path, daemon_home: &Path, raw: &str) -> Result<Dest, String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err("Pick a folder to clone into.".to_string());
    }
    let path = match raw.strip_prefix("~/").or_else(|| raw.strip_prefix("~\\")) {
        Some(rest) => user_home.join(rest),
        None if raw == "~" => user_home.to_path_buf(),
        None => PathBuf::from(raw),
    };
    if !path.is_absolute() {
        return Err(format!(
            "'{raw}' isn't a full path: start it with / or ~/ (your home folder)."
        ));
    }
    if path
        .components()
        .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
    {
        return Err(format!("Give '{raw}' without . or .. in it."));
    }
    // The nearest entry that exists, a symlink included (a dangling one
    // fails to resolve below rather than being written through).
    let mut base = path.clone();
    let mut below = Vec::new();
    loop {
        match std::fs::symlink_metadata(&base) {
            Ok(_) => break,
            Err(e) if e.kind() == ErrorKind::NotFound => {
                let (Some(name), Some(parent)) = (base.file_name(), base.parent()) else {
                    return Err(format!("Can't find where '{raw}' is."));
                };
                below.push(name.to_os_string());
                base = parent.to_path_buf();
            }
            Err(e) => return Err(format!("Can't read '{}': {e}.", base.display())),
        }
    }
    let base = safe_git::canonical(&base)
        .map_err(|e| format!("Can't resolve '{}': {e}.", base.display()))?;
    let resolved = below
        .iter()
        .rev()
        .fold(base.clone(), |p, name| p.join(name));
    let home = safe_git::canonical(user_home)
        .map_err(|e| format!("Can't resolve your home folder: {e}."))?;
    if resolved == home || !resolved.starts_with(&home) {
        return Err(format!(
            "Clone into a folder inside your home folder ({}), not the home itself.",
            home.display()
        ));
    }
    // The daemon's folder sits in the home; a home inside it (a test's)
    // leaves nothing to refuse.
    if safe_git::canonical(daemon_home)
        .is_ok_and(|own| resolved.starts_with(&own) && !home.starts_with(&own))
    {
        return Err("Pick a folder outside Flotilla's own.".to_string());
    }
    let existed = below.is_empty();
    if existed {
        let empty = std::fs::read_dir(&resolved)
            .map(|mut entries| entries.next().is_none())
            .unwrap_or(false);
        if !empty {
            return Err(format!(
                "{} already exists: pick a new folder, or an empty one.",
                resolved.display()
            ));
        }
    }
    Ok(Dest {
        path: resolved,
        base,
        existed,
    })
}

/// Runs the clone in the background, its slot already claimed with
/// [`Clones::begin`], and tells every client how it goes.
pub fn spawn(
    app: Arc<AppState>,
    project_id: String,
    url: String,
    branch: Option<String>,
    dest: Dest,
) {
    tokio::spawn(async move {
        push(&app, &project_id, "cloning", None, None);
        let (u, b, d) = (url.clone(), branch.clone(), dest.clone());
        let cloned = tokio::task::spawn_blocking(move || clone(&u, b.as_deref(), &d))
            .await
            .unwrap_or_else(|e| Err(format!("the clone stopped: {e}")));
        match cloned {
            Ok(()) => {
                tracing::info!(project_id, path = %dest.path.display(), "repository cloned");
                let path = dest.path.display().to_string();
                push(&app, &project_id, "done", Some(path), None);
            }
            Err(reason) => {
                tracing::warn!(project_id, %reason, "repository clone failed");
                remove(&dest);
                let error = crate::redact::secrets(&format!("Couldn't clone {url}: {reason}"));
                push(&app, &project_id, "failed", None, Some(error));
            }
        }
        app.repo_clones.finish(&project_id);
    });
}

fn push(
    app: &AppState,
    project_id: &str,
    state: &str,
    path: Option<String>,
    error: Option<String>,
) {
    app.events.push(Push::RepoClone {
        project_id: project_id.to_string(),
        state: state.to_string(),
        path,
        error,
    });
}

/// `git clone` into the folder; git's last word when it fails.
fn clone(url: &str, branch: Option<&str>, dest: &Dest) -> Result<(), String> {
    let target = dest.path.display().to_string();
    let mut args = vec!["clone", "--quiet"];
    if let Some(branch) = branch {
        args.extend(["--branch", branch]);
    }
    args.extend(["--", url, &target]);
    let mut git = SafeGit::fetching(&dest.base, url).map_err(|e| format!("{e:#}"))?;
    for kv in LOW_SPEED {
        git = git.configured(kv);
    }
    let out = git.args(&args).output().map_err(|e| format!("{e:#}"))?;
    if out.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&out.stderr);
    let reason = stderr
        .lines()
        .map(str::trim)
        .rfind(|line| !line.is_empty())
        .unwrap_or("git gave no reason");
    let reason = reason
        .strip_prefix("fatal: ")
        .or_else(|| reason.strip_prefix("error: "))
        .unwrap_or(reason);
    Err(format!("{}.", reason.trim_end_matches('.')))
}

/// Removes what a failed clone made: the folder's contents when it was
/// there before, else the folder and any empty one git made above it.
fn remove(dest: &Dest) {
    if dest.existed {
        if let Ok(entries) = std::fs::read_dir(&dest.path) {
            for entry in entries.flatten() {
                let path = entry.path();
                let gone = if entry.file_type().is_ok_and(|t| t.is_dir()) {
                    std::fs::remove_dir_all(&path)
                } else {
                    std::fs::remove_file(&path)
                };
                if let Err(e) = gone {
                    tracing::warn!(path = %path.display(), error = %e, "clone leftover not removed");
                }
            }
        }
        return;
    }
    if dest.path.exists() {
        if let Err(e) = std::fs::remove_dir_all(&dest.path) {
            tracing::warn!(path = %dest.path.display(), error = %e, "clone leftover not removed");
        }
    }
    let mut above = dest.path.parent();
    while let Some(dir) = above.filter(|d| *d != dest.base && d.starts_with(&dest.base)) {
        if std::fs::remove_dir(dir).is_err() {
            break;
        }
        above = dir.parent();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn homes() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let root = tempfile::tempdir().unwrap();
        let home = safe_git::canonical(root.path()).unwrap();
        let daemon = home.join(".flotilla");
        std::fs::create_dir_all(&daemon).unwrap();
        (root, home, daemon)
    }

    #[test]
    fn a_folder_in_the_home_is_a_destination() {
        let (_root, home, daemon) = homes();
        let dest = destination(&home, &daemon, "~/code/app").unwrap();
        assert_eq!(dest.path, home.join("code/app"));
        assert_eq!(dest.base, home);
        assert!(!dest.existed);
        let absolute = home.join("app").display().to_string();
        assert_eq!(
            destination(&home, &daemon, &absolute).unwrap().path,
            home.join("app")
        );
        std::fs::create_dir(home.join("empty")).unwrap();
        let empty = destination(&home, &daemon, "~/empty").unwrap();
        assert!(empty.existed);
    }

    #[test]
    fn other_folders_are_refused() {
        let (_root, home, daemon) = homes();
        std::fs::create_dir(home.join("full")).unwrap();
        std::fs::write(home.join("full/file"), "x").unwrap();
        let outside = tempfile::tempdir().unwrap();
        let outside = outside.path().join("app").display().to_string();
        for bad in [
            "",
            "~",
            "~/",
            "code/app",
            "~/code/../app",
            "~/full",
            "~/.flotilla/app",
            &outside,
        ] {
            assert!(destination(&home, &daemon, bad).is_err(), "{bad:?}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_are_followed_out_of_the_home() {
        let (_root, home, daemon) = homes();
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), home.join("out")).unwrap();
        assert!(destination(&home, &daemon, "~/out/app").is_err());
        std::os::unix::fs::symlink(home.join("missing"), home.join("dangling")).unwrap();
        assert!(destination(&home, &daemon, "~/dangling/app").is_err());
        std::fs::create_dir(home.join("real")).unwrap();
        std::os::unix::fs::symlink(home.join("real"), home.join("alias")).unwrap();
        let dest = destination(&home, &daemon, "~/alias/app").unwrap();
        assert_eq!(dest.path, home.join("real/app"));
    }

    #[test]
    fn only_https_and_local_urls_clone() {
        for good in [
            "https://github.com/me/app.git",
            "file:///srv/app.git",
            "/srv/app.git",
        ] {
            assert!(check_url(good).is_ok(), "{good}");
        }
        for bad in [
            "git@github.com:me/app.git",
            "ssh://host/app",
            "http://host/app",
            "git://h/a",
        ] {
            assert!(check_url(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn a_failed_clone_leaves_nothing() {
        let (_root, home, daemon) = homes();
        let dest = destination(&home, &daemon, "~/a/b/app").unwrap();
        std::fs::create_dir_all(dest.path.join("partial")).unwrap();
        remove(&dest);
        assert!(!home.join("a").exists());
        std::fs::create_dir(home.join("kept")).unwrap();
        let kept = destination(&home, &daemon, "~/kept").unwrap();
        std::fs::write(home.join("kept/partial"), "x").unwrap();
        remove(&kept);
        assert!(home.join("kept").is_dir());
        assert!(std::fs::read_dir(home.join("kept"))
            .unwrap()
            .next()
            .is_none());
    }
}
