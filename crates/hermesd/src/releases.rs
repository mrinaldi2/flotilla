//! Release packages and the deploy gate (H-020 §2, §6; H-017 §1.4).
//!
//! DevOps assembles a package (`assemble`), the owner rules on it from the
//! dashboard or phone (`rule`), and DevOps rolls it out through each
//! machine's tester (`deploy`). The gate is that only `rule`, called with the
//! owner's own credentials on the board's home, can move items to deploying,
//! and `deploy` re-checks the ruling every time it is used.

use std::sync::Arc;

use sha2::{Digest, Sha256};

use crate::actor::Actor;
use crate::app::AppState;
use crate::db::BoardTx;
use crate::decisions::{conflict, forbidden, not_found};
use crate::kanban::model::Role;

use model::Release;

pub mod assemble;
pub mod blockers;
pub mod blockers_watch;
pub mod build_installer;
pub mod cancel;
pub mod cli;
pub mod confine;
mod contains;
pub mod cut;
pub mod deploy;
pub mod deployed_via;
pub mod gates;
pub mod git;
pub mod git_cache;
pub mod install;
pub mod ios_repair;
pub mod land;
pub mod leave_out;
pub mod leave_out_cli;
pub mod leave_out_gate;
pub mod lifecycle;
pub mod machines;
pub mod model;
pub mod package;
pub mod phone;
pub mod phone_version;
pub mod plan;
pub mod post_install;
pub mod publish;
pub mod quiesce_gate;
pub mod rule;
pub mod serve;
pub mod sha_cache;
mod supersede;
pub mod tag;
pub mod tag_cli;
#[cfg(test)]
mod tests;

/// A bot acting on releases, with its board roles.
pub struct Caller<'a> {
    pub bot: &'a bus::Bot,
    pub roles: Vec<Role>,
}

impl Caller<'_> {
    pub fn load<'a>(app: &Arc<AppState>, bot: &'a bus::Bot) -> anyhow::Result<Caller<'a>> {
        let roles = app
            .db
            .project_roles(&bot.project_id)?
            .into_iter()
            .filter(|r| r.bot_id == bot.id)
            .map(|r| r.role)
            .collect();
        Ok(Caller { bot, roles })
    }

    pub fn actor(&self) -> Actor<'_> {
        Actor::Bot {
            id: &self.bot.id,
            project_id: &self.bot.project_id,
        }
    }

    pub fn has(&self, role: Role) -> bool {
        self.roles.contains(&role)
    }

    /// Refused unless the bot holds `role`.
    pub fn require(&self, role: Role, what: &str) -> anyhow::Result<()> {
        if self.has(role) {
            Ok(())
        } else {
            Err(forbidden(format!(
                "only a bot with the {} role can {what}; ask the lead to assign it",
                role.as_str()
            )))
        }
    }
}

/// A package of the caller's project; another project's packages don't
/// exist for it.
pub fn load(t: &BoardTx<'_>, project_id: &str, id: &str) -> anyhow::Result<Release> {
    t.release(id)?
        .filter(|r| r.project_id == project_id)
        .ok_or_else(|| not_found(format!("no release {id} in this project")))
}

/// What a package is, for the gate: its items, its builds and the tests run
/// against them. Taken at submit and compared at the ruling and at every
/// deploy, so a package can't change under an approval.
pub fn frozen_hash(release: &Release) -> String {
    let mut h = Sha256::new();
    let mut items: Vec<&str> = release.items.iter().map(|i| i.item_id.as_str()).collect();
    items.sort_unstable();
    for item in items {
        h.update(format!("item\0{item}\n"));
    }
    for b in &release.builds {
        h.update(format!(
            "build\0{}\0{}\0{}\n",
            b.platform, b.version, b.sha256
        ));
        // Only when recorded, so packages frozen before it keep their hash.
        if let Some(commit) = &b.source_commit {
            h.update(format!("commit\0{}\0{commit}\n", b.platform));
        }
    }
    for t in &release.tests {
        h.update(format!(
            "test\0{}\0{}\0{}\n",
            t.machine, t.build_sha256, t.result
        ));
    }
    // The frozen computer sets (ARCH-R55 M1); none on older packages, so
    // their hash stays.
    let targets = &release.targets;
    for (kind, machines) in [
        ("test", &targets.tested_on),
        ("deploy", &targets.deploys_to),
    ] {
        for m in machines {
            h.update(format!("target\0{kind}\0{m}\n"));
        }
    }
    hex::encode(h.finalize())
}

/// Refused with "resubmit" when the package changed since it was frozen.
pub fn check_frozen(release: &Release) -> anyhow::Result<()> {
    match &release.frozen_hash {
        Some(hash) if *hash == frozen_hash(release) => Ok(()),
        Some(_) => Err(conflict(format!(
            "release {} changed after it was submitted; DevOps must resubmit it",
            release.name
        ))),
        None => Err(conflict(format!(
            "release {} has not been submitted",
            release.name
        ))),
    }
}

/// The bot ids of a project's testers on `machine`, a computer as
/// [`machines::testers`] names it (H-115). The owner's iPhone has no tester
/// of its own: iOS QA, the tester on `ios`, carries its deploys (H-176).
pub fn testers_on(
    app: &Arc<AppState>,
    project_id: &str,
    machine: &str,
) -> anyhow::Result<Vec<String>> {
    let ios = machines::is_ios_target(machine);
    Ok(app
        .db
        .board_read(|t| machines::testers(t, project_id))?
        .into_iter()
        .filter(|(_, m)| m.eq_ignore_ascii_case(machine) || (ios && machines::is_ios_target(m)))
        .map(|(bot, _)| bot)
        .collect())
}
