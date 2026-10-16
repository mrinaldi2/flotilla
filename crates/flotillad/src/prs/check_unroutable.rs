//! Checks no computer can run (H-318): routing found no reported computer
//! with the check's `needs` on its `machine`. The check stays queued with
//! why on it, so the next probe (which nudges dispatch) routes it once a
//! computer reports the tool. A required one unroutable for
//! [`NEEDS_YOU_AFTER`] is a Needs-you row for the owner and a note to the
//! project's DevOps, once per check name. The merge gate is unchanged: an
//! unroutable required check isn't a pass.

use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};

use chrono::{Duration, Utc};

use crate::app::AppState;
use crate::kanban::model::Role;
use crate::prs::check_model::{CheckRun, Unroutable};

/// How long a required check is unroutable before someone is asked.
pub const NEEDS_YOU_AFTER: Duration = Duration::hours(1);

/// Keeps why no computer can run `check`, and since when (the first time).
pub fn mark(app: &AppState, check: &CheckRun, why: &str) {
    if let Err(error) = app.db.board_tx(|t| t.set_check_unroutable(&check.id, why)) {
        tracing::warn!(check = %check.name, %error, "unroutable check not kept");
    }
}

/// `(project, check name)` DevOps has been told about while it stays
/// unroutable. Lost at a restart, which tells once more.
fn told() -> &'static Mutex<HashSet<(String, String)>> {
    static TOLD: OnceLock<Mutex<HashSet<(String, String)>>> = OnceLock::new();
    TOLD.get_or_init(Mutex::default)
}

/// Tells the project's DevOps, once per check name, about each required
/// check unroutable for [`NEEDS_YOU_AFTER`]; one that routes again is
/// forgotten, so it is told again if it comes back.
pub fn tell_devops(app: &AppState, project: &str) {
    let Ok(stuck) = app
        .db
        .board_read(|t| t.unroutable_checks(project, Utc::now() - NEEDS_YOU_AFTER))
    else {
        return;
    };
    let mut told = told().lock().unwrap_or_else(|e| e.into_inner());
    told.retain(|(p, name)| p != project || stuck.iter().any(|s| &s.name == name));
    let devops = app.db.project_roles(project).ok().and_then(|roles| {
        roles
            .into_iter()
            .find(|r| r.role == Role::Devops)
            .map(|r| r.bot_id)
    });
    let Some(devops) = devops else { return };
    for Unroutable { name, note, pr, .. } in stuck {
        if !told.insert((project.to_string(), name.clone())) {
            continue;
        }
        let body = format!(
            "Check {name} on PR #{pr} has had no computer to run it for over an hour: {note}. \
             Install the tool on a computer (it routes after the next probe), or change the \
             check's needs/machine in a PR."
        );
        let sender = crate::messaging::daemon_sender();
        let dm = crate::messaging::Dm::new(&devops, &sender, bus::MessageKind::Note, &body);
        if let Err(error) = crate::messaging::send_dm(&app.db, &app.events, dm) {
            tracing::warn!(check = %name, %error, "unroutable check note to DevOps failed");
        }
    }
}
