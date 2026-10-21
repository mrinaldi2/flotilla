//! A PR's repository as the daemon reads it (H-261 §1.2, §3): its own
//! blob-less cache, never a bot's checkout, so a head is what the remote
//! holds and its patch-id is the PR's own change.

use std::path::PathBuf;

use crate::app::AppState;
use crate::releases::git::github_https;
use crate::releases::git_cache;

/// A repository a project's PRs live in.
pub struct Repo {
    /// "owner/name" (GitHub), or the URL for any other remote.
    pub name: String,
    pub url: String,
    /// The branch its PRs land on: the project's own repository's branch
    /// from its settings; another repository's default branch.
    pub base: Base,
    /// The cache's key: the project's own repository keeps the release
    /// cache; another shares nothing with it.
    key: String,
}

/// Where a repository's PRs land.
pub enum Base {
    /// The branch the owner set for the project's repository.
    Branch(String),
    /// The remote's default branch, as the cache's HEAD names it.
    Default,
}

impl Base {
    fn of(branch: &str) -> Self {
        match branch.trim() {
            "" => Self::Branch("main".into()),
            b => Self::Branch(b.into()),
        }
    }
}

/// "owner/name" for a GitHub URL; any other URL as itself, trimmed.
pub fn name_of(url: &str) -> String {
    let url = url.trim().trim_end_matches('/');
    let url = url.strip_suffix(".git").unwrap_or(url);
    match github_https(url) {
        Some(https) => https.trim_start_matches("https://github.com/").to_string(),
        None => url.to_string(),
    }
}

/// Whether two remote URLs name the same repository, however spelled
/// (`git@github.com:o/r.git`, `https://github.com/o/r`).
pub fn same(a: &str, b: &str) -> bool {
    name_of(a).eq_ignore_ascii_case(&name_of(b))
}

/// The repository `asked` names ("owner/name"), or the project's own. Only
/// the project's repository or one the owner added (`project_repo_extra`) is
/// ever fetched: a bot can't point the daemon at any other (ARCH M1).
pub fn of_project(app: &AppState, project_id: &str, asked: Option<&str>) -> anyhow::Result<Repo> {
    let own = app
        .db
        .project_repo(project_id)?
        .map(|r| (r.url, Base::of(&r.branch)));
    let asked = asked.map(str::trim).filter(|a| !a.is_empty());
    match (own, asked) {
        (Some((url, base)), None) => Ok(Repo {
            name: name_of(&url),
            url,
            base,
            key: project_id.to_string(),
        }),
        (Some((url, base)), Some(name)) if same(&url, name) => Ok(Repo {
            name: name_of(&url),
            url,
            base,
            key: project_id.to_string(),
        }),
        (None, None) => anyhow::bail!(
            "the project has no repository set; the owner sets it in the project's settings"
        ),
        (_, Some(name)) => {
            let url = app
                .db
                .extra_repos(project_id)?
                .into_iter()
                .find(|url| same(url, name))
                .ok_or_else(|| {
                    crate::decisions::forbidden(format!(
                        "{name} isn't one of this project's repositories; the owner adds it \
                         in the project's settings"
                    ))
                })?;
            let repo = name_of(&url);
            Ok(Repo {
                key: format!("{project_id}--{}", repo.replace(['/', ':', '\\'], "--")),
                name: repo,
                url,
                base: Base::Default,
            })
        }
    }
}

impl Repo {
    /// The daemon's cache of it, fetched now.
    pub fn fetch(&self, app: &AppState) -> anyhow::Result<PathBuf> {
        git_cache::refresh(&app.cfg.home, &self.key, &self.url)
    }

    /// The cache without fetching, if it exists.
    pub fn cached(&self, app: &AppState) -> PathBuf {
        git_cache::dir(&app.cfg.home, &self.key)
    }

    /// The branch its PRs land on, by name ("develop"), when the owner set it.
    pub fn base_branch(&self) -> Option<&str> {
        match &self.base {
            Base::Branch(b) => Some(b),
            Base::Default => None,
        }
    }

    /// That branch's name, reading the cache for a remote's default.
    pub fn base_name(&self, cache: &std::path::Path) -> Option<String> {
        match &self.base {
            Base::Branch(b) => Some(b.clone()),
            // The bare cache's HEAD: the branch the remote's HEAD named.
            Base::Default => crate::releases::git::git(cache, &["symbolic-ref", "--quiet", "HEAD"])
                .ok()
                .and_then(|r| r.strip_prefix("refs/heads/").map(str::to_string)),
        }
    }

    /// The ref of that branch in the cache.
    pub fn base_ref(&self) -> String {
        match &self.base {
            Base::Branch(b) => format!("refs/heads/{b}"),
            Base::Default => "HEAD".into(),
        }
    }

    /// The base branch's tip in the cache, or why there is none.
    pub fn base_tip(&self, cache: &std::path::Path) -> anyhow::Result<String> {
        git_cache::resolve(cache, &self.base_ref()).ok_or_else(|| match &self.base {
            Base::Branch(b) => anyhow::anyhow!(
                "{} has no {b} branch; the owner sets the branch PRs land on in the \
                 project's settings",
                self.name
            ),
            Base::Default => anyhow::anyhow!("{} has no default branch", self.name),
        })
    }

    /// The head's [`Facts`] against the base branch.
    pub fn facts(&self, cache: &std::path::Path, head: &str) -> anyhow::Result<Facts> {
        let main = self.base_tip(cache)?;
        let base = git_cache::merge_base(cache, &main, head)?;
        Ok(Facts {
            base_sha: main,
            patch_id: git_cache::patch_id(cache, &base, head)?,
        })
    }
}

/// A head's facts in the cache: the base branch's tip, and the patch-id of
/// the change from its merge-base with that tip to the head (§3).
pub struct Facts {
    pub base_sha: String,
    pub patch_id: String,
}

/// The branch's tip on the remote, as fetched into the cache.
pub fn tip(cache: &std::path::Path, branch: &str) -> Option<String> {
    git_cache::resolve(cache, &format!("refs/heads/{branch}"))
}
