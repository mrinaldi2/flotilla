//! A project's mirror (0.23.0, `repo_mirror_v1`): one bare repository per
//! project at `<home>/repos/<project dir>.git`, holding every object once
//! for all of the project's card worktrees here. The remote's branches are
//! kept as `refs/remotes/origin/*`, so a fetch never moves a branch a
//! worktree has checked out, and they aren't pruned, so work pushed to a
//! branch since deleted still counts as saved. Fetched on a timer and before
//! each new worktree, through the hardened runner (H-289): bots commit in
//! its worktrees, so its config is theirs to write.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, SystemTime};

use crate::app::AppState;
use crate::releases::git::github_https;
use crate::safe_git::SafeGit;

/// The remote's branches, as remote-tracking refs.
pub const TRACKING: &str = "+refs/heads/*:refs/remotes/origin/*";
const TAGS: &str = "+refs/tags/*:refs/tags/*";
/// A read fetches first when the last fetch is older than this.
pub const STALE: Duration = Duration::from_secs(300);

/// `<home>/repos/<project dir>.git`.
pub fn path(home: &Path, project_dir: &str) -> PathBuf {
    home.join("repos").join(format!("{project_dir}.git"))
}

/// The mirror of a project's repository on this computer.
#[derive(Debug, Clone)]
pub struct Mirror {
    pub project_id: String,
    pub project_dir: String,
    pub path: PathBuf,
    /// The repository's URL as the project names it; bots push there.
    pub url: String,
    /// The branch the project names, which may not exist (see [`Mirror::base`]).
    pub branch: String,
}

pub(crate) fn git(dir: &Path, args: &[&str]) -> anyhow::Result<String> {
    SafeGit::local(dir)?.args(args).run()
}

/// A mirror's lock, by its path.
type Locks = Mutex<Vec<(PathBuf, Arc<Mutex<()>>)>>;

/// One lock per mirror: a fetch and a new worktree never race on its refs.
fn lock_of(path: &Path) -> Arc<Mutex<()>> {
    static LOCKS: OnceLock<Locks> = OnceLock::new();
    let mut locks = LOCKS
        .get_or_init(Mutex::default)
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if let Some((_, lock)) = locks.iter().find(|(p, _)| p == path) {
        return lock.clone();
    }
    let lock = Arc::new(Mutex::new(()));
    locks.push((path.to_path_buf(), lock.clone()));
    lock
}

impl Mirror {
    /// The mirror of `project_id`'s repository, or why it has none.
    pub fn of_project(app: &AppState, project_id: &str) -> anyhow::Result<Self> {
        let project = app
            .db
            .get_project(project_id)?
            .ok_or_else(|| anyhow::anyhow!("no project {project_id}"))?;
        let repo = app.db.project_repo(project_id)?.ok_or_else(|| {
            anyhow::anyhow!(
                "the project has no repository set; the owner sets it in the project's settings"
            )
        })?;
        Ok(Self {
            project_id: project_id.to_string(),
            path: path(&app.cfg.home, &project.dir_name),
            project_dir: project.dir_name,
            url: repo.url,
            branch: repo.branch,
        })
    }

    pub fn exists(&self) -> bool {
        self.path.join("HEAD").is_file()
    }

    /// Held while fetching or adding a worktree.
    pub fn lock(&self) -> Arc<Mutex<()>> {
        lock_of(&self.path)
    }

    fn fetch_url(&self) -> String {
        github_https(&self.url).unwrap_or_else(|| self.url.clone())
    }

    fn create(&self) -> anyhow::Result<()> {
        let parent = self.path.parent().expect("a mirror has a parent");
        std::fs::create_dir_all(parent)?;
        let target = self.path.display().to_string();
        git(parent, &["init", "--quiet", "--bare", &target])?;
        git(&self.path, &["config", "remote.origin.fetch", TRACKING])?;
        Ok(())
    }

    /// Creates the mirror if it isn't there, then fetches every branch and
    /// tag, and the remote's default branch.
    pub fn fetch(&self) -> anyhow::Result<()> {
        let lock = self.lock();
        let _held = lock.lock().unwrap_or_else(|e| e.into_inner());
        self.fetch_locked()
    }

    /// [`Mirror::fetch`] for a caller already holding [`Mirror::lock`].
    pub fn fetch_locked(&self) -> anyhow::Result<()> {
        if !self.exists() {
            self.create()?;
        }
        // Bots' own `git fetch` and `git push` in a worktree use this URL.
        git(&self.path, &["config", "remote.origin.url", &self.url])?;
        let url = self.fetch_url();
        SafeGit::fetching(&self.path, &url)?
            .args(&["fetch", "--quiet", "--no-tags", &url, TRACKING, TAGS])
            .run()?;
        let listed = SafeGit::fetching(&self.path, &url)?
            .args(&["ls-remote", "--symref", &url, "HEAD"])
            .run()
            .unwrap_or_default();
        if let Some(default) = default_branch(&listed) {
            let target = format!("refs/remotes/origin/{default}");
            if self.resolve(&target).is_some() {
                git(
                    &self.path,
                    &["symbolic-ref", "refs/remotes/origin/HEAD", &target],
                )?;
            }
        }
        Ok(())
    }

    /// Fetches when the mirror is missing or its last fetch is older than
    /// [`STALE`]. A failed fetch of a mirror that exists reads what it has.
    pub fn fresh(&self) -> anyhow::Result<()> {
        if !self.exists() {
            return self.fetch();
        }
        let fetched = std::fs::metadata(self.path.join("FETCH_HEAD"))
            .and_then(|m| m.modified())
            .ok()
            .and_then(|at| SystemTime::now().duration_since(at).ok());
        if fetched.is_none_or(|age| age > STALE) {
            if let Err(error) = self.fetch() {
                tracing::warn!(mirror = %self.path.display(), %error, "mirror fetch failed");
            }
        }
        Ok(())
    }

    /// The commit `reference` names here, if it names one.
    pub fn resolve(&self, reference: &str) -> Option<String> {
        git(
            &self.path,
            &[
                "rev-parse",
                "--verify",
                "--quiet",
                "--end-of-options",
                &format!("{reference}^{{commit}}"),
            ],
        )
        .ok()
        .filter(|c| !c.is_empty())
    }

    /// The branch new work starts from: the project's, or the remote's
    /// default branch when the project's doesn't exist there (a project
    /// set to `main` whose repository only has `develop`).
    pub fn base(&self) -> anyhow::Result<String> {
        if self
            .resolve(&format!("refs/remotes/origin/{}", self.branch))
            .is_some()
        {
            return Ok(self.branch.clone());
        }
        let head = git(
            &self.path,
            &["symbolic-ref", "--quiet", "refs/remotes/origin/HEAD"],
        )
        .unwrap_or_default();
        match head.strip_prefix("refs/remotes/origin/") {
            Some(default) if !default.is_empty() => Ok(default.to_string()),
            _ => anyhow::bail!(
                "the repository has no branch {} and names no default branch",
                self.branch
            ),
        }
    }
}

/// The branch `git ls-remote --symref <url> HEAD` names.
pub fn default_branch(listed: &str) -> Option<&str> {
    listed.lines().find_map(|line| {
        let (target, name) = line.strip_prefix("ref: ")?.split_once('\t')?;
        (name == "HEAD").then(|| target.strip_prefix("refs/heads/"))?
    })
}
