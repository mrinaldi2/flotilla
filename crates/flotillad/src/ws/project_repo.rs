//! A project's shared git repository, which workers check out and push to.

use serde_json::{json, Value};

use super::Conn;
use crate::botmgmt;

/// The requests on a project's repositories.
pub(super) const KINDS: &[&str] = &[
    "set_project_repo",
    "set_project_extra_repos",
    "clone_project_repo",
    "set_project_disk_budget",
    "set_project_pr_flow",
];

impl Conn {
    pub(super) fn repo_request(
        &self,
        kind: &str,
        req_id: &Value,
        req: &Value,
    ) -> anyhow::Result<()> {
        match kind {
            "set_project_repo" => self.set_project_repo(req_id, req),
            "set_project_extra_repos" => self.set_project_extra_repos(req_id, req),
            "set_project_disk_budget" => self.set_project_disk_budget(req_id, req),
            "set_project_pr_flow" => self.set_project_pr_flow(req_id, req),
            _ => self.clone_project_repo(req_id, req),
        }
    }

    /// Set the repository with `url` (and `branch`, default `main`), or clear
    /// it with `url: null`. Bots' system prompts are rewritten to match, for
    /// their next start.
    pub(super) fn set_project_repo(&self, req_id: &Value, req: &Value) -> anyhow::Result<()> {
        let project_id = Self::str_field(req, "project_id")?;
        let Some(project) = self.app.db.get_live_project(project_id)? else {
            self.reply_err(req_id, "not_found", "project not found or already deleted");
            return Ok(());
        };
        let repo = match req.get("url").and_then(Value::as_str) {
            Some(url) => {
                let branch = req.get("branch").and_then(Value::as_str);
                match bus::ProjectRepo::parse(url, branch) {
                    Ok(repo) => Some(repo),
                    Err(message) => {
                        self.reply_err(req_id, "invalid_request", &message);
                        return Ok(());
                    }
                }
            }
            None => None,
        };
        self.app.db.set_project_repo(&project.id, repo.as_ref())?;
        for bot in self
            .app
            .db
            .list_bots(Some(&project.id))?
            .into_iter()
            .filter(|b| !b.is_linked())
        {
            if let Err(error) = botmgmt::reprovision(&self.app, &bot) {
                tracing::warn!(bot_id = %bot.id, %error, "system.md not rewritten for the repository");
            }
        }
        self.app.events.push(crate::events::Push::ProjectUpdated {
            project: project.clone(),
        });
        self.send(json!({
            "type": "project", "req_id": req_id,
            "project": super::project_view(&self.app, &project)
        }));
        Ok(())
    }

    /// `set_project_extra_repos {project_id, urls}`: the other repositories
    /// this project's PRs may live in (H-266, ARCH M1). The owner's alone:
    /// approve, from a device or the app's ticket, never the owner token, and
    /// no bot tool sets it. Answers with the project and its `extra_repos`.
    pub(super) fn set_project_extra_repos(
        &self,
        req_id: &Value,
        req: &Value,
    ) -> anyhow::Result<()> {
        let project_id = Self::str_field(req, "project_id")?;
        let Some(project) = self.app.db.get_live_project(project_id)? else {
            self.reply_err(req_id, "not_found", "project not found or already deleted");
            return Ok(());
        };
        let Some(proof) = self.owner_proof() else {
            self.reply_err(
                req_id,
                "forbidden",
                "only the owner's app or a paired device sets this",
            );
            return Ok(());
        };
        let mut urls = Vec::new();
        for url in req
            .get("urls")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let Some(url) = url.as_str().map(str::trim).filter(|u| !u.is_empty()) else {
                self.reply_err(req_id, "invalid_request", "urls must be repository URLs");
                return Ok(());
            };
            if let Err(message) = bus::ProjectRepo::parse(url, None) {
                self.reply_err(req_id, "invalid_request", &message);
                return Ok(());
            }
            urls.push(url.to_string());
        }
        let by = match proof {
            crate::db::OwnerProof::Device { device_id } => format!("device:{device_id}"),
            crate::db::OwnerProof::Ticket => "ticket".to_string(),
            crate::db::OwnerProof::Peer { .. } => {
                self.reply_err(
                    req_id,
                    "forbidden",
                    "set this on the project's own computer",
                );
                return Ok(());
            }
        };
        self.app.db.set_extra_repos(&project.id, &urls, &by)?;
        let mut view = super::project_view(&self.app, &project);
        view["extra_repos"] = json!(self.app.db.extra_repos(&project.id)?);
        self.send(json!({ "type": "project", "req_id": req_id, "project": view }));
        Ok(())
    }

    /// `set_project_disk_budget {project_id, bytes | null}` (0.23.0): what
    /// the project's worktrees and build caches may use on this computer;
    /// `null` takes the budget away. The owner's alone, like the extra
    /// repositories: approve, from a device or the app's ticket.
    pub(super) fn set_project_disk_budget(
        &self,
        req_id: &Value,
        req: &Value,
    ) -> anyhow::Result<()> {
        let project_id = Self::str_field(req, "project_id")?;
        let Some(project) = self.app.db.get_live_project(project_id)? else {
            self.reply_err(req_id, "not_found", "project not found or already deleted");
            return Ok(());
        };
        match self.owner_proof() {
            Some(crate::db::OwnerProof::Device { .. } | crate::db::OwnerProof::Ticket) => {}
            _ => {
                self.reply_err(
                    req_id,
                    "forbidden",
                    "only the owner's app or a paired device sets this, on the project's own computer",
                );
                return Ok(());
            }
        }
        let bytes = match req.get("bytes") {
            None | Some(Value::Null) => None,
            Some(v) => match v.as_u64().filter(|b| *b > 0 && i64::try_from(*b).is_ok()) {
                Some(b) => Some(b),
                None => {
                    self.reply_err(
                        req_id,
                        "invalid_request",
                        "bytes must be a positive number or null",
                    );
                    return Ok(());
                }
            },
        };
        self.app.db.set_project_disk_budget(&project.id, bytes)?;
        self.app.events.push(crate::events::Push::ProjectUpdated {
            project: project.clone(),
        });
        self.send(json!({
            "type": "project", "req_id": req_id,
            "project": super::project_view(&self.app, &project)
        }));
        Ok(())
    }

    /// `set_project_pr_flow {project_id, enabled}` (0.23.1): the project's
    /// code goes through pull requests. Bots' prompts carry the PR section
    /// from their next start. The owner's alone, like the disk budget.
    pub(super) fn set_project_pr_flow(&self, req_id: &Value, req: &Value) -> anyhow::Result<()> {
        let project_id = Self::str_field(req, "project_id")?;
        let Some(project) = self.app.db.get_live_project(project_id)? else {
            self.reply_err(req_id, "not_found", "project not found or already deleted");
            return Ok(());
        };
        if !matches!(
            self.owner_proof(),
            Some(crate::db::OwnerProof::Device { .. } | crate::db::OwnerProof::Ticket)
        ) {
            self.reply_err(
                req_id,
                "forbidden",
                "only the owner's app or a paired device sets this, on the project's own computer",
            );
            return Ok(());
        }
        let Some(enabled) = req.get("enabled").and_then(Value::as_bool) else {
            self.reply_err(req_id, "invalid_request", "enabled must be true or false");
            return Ok(());
        };
        self.app.db.set_project_pr_flow(&project.id, enabled)?;
        self.app.events.push(crate::events::Push::ProjectUpdated {
            project: project.clone(),
        });
        self.send(json!({
            "type": "project", "req_id": req_id,
            "project": super::project_view(&self.app, &project)
        }));
        Ok(())
    }

    /// `clone_project_repo {project_id, url, branch?, dest}` (0.20.0): the
    /// owner's clone into a folder in their home, run in the background;
    /// see [`crate::repo_clone`]. It leaves the project's `repo` as it is.
    pub(super) fn clone_project_repo(&self, req_id: &Value, req: &Value) -> anyhow::Result<()> {
        let project_id = Self::str_field(req, "project_id")?;
        if self.app.db.get_live_project(project_id)?.is_none() {
            self.reply_err(req_id, "not_found", "project not found or already deleted");
            return Ok(());
        }
        if matches!(self.owner_proof(), Some(crate::db::OwnerProof::Peer { .. })) {
            self.reply_err(req_id, "forbidden", "clone on the project's own computer");
            return Ok(());
        }
        let url = Self::str_field(req, "url")?.trim().to_string();
        let branch = req
            .get("branch")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|b| !b.is_empty());
        let checked = bus::ProjectRepo::parse(&url, branch)
            .and_then(|_| crate::repo_clone::check_url(&url))
            .and_then(|()| {
                crate::repo_clone::destination(
                    &self.app.cfg.user_home,
                    &self.app.cfg.home,
                    Self::str_field(req, "dest").unwrap_or_default(),
                )
            });
        let dest = match checked {
            Ok(dest) => dest,
            Err(message) => {
                self.reply_err(req_id, "invalid_request", &message);
                return Ok(());
            }
        };
        if !self.app.repo_clones.begin(project_id) {
            self.reply_err(
                req_id,
                "conflict",
                "a clone for this project is already running",
            );
            return Ok(());
        }
        self.send(
            json!({ "type": "repo_clone_started", "req_id": req_id, "project_id": project_id }),
        );
        crate::repo_clone::spawn(
            self.app.clone(),
            project_id.to_string(),
            url,
            branch.map(str::to_string),
            dest,
        );
        Ok(())
    }
}
