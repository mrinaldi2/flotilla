//! Jobs a linked computer lost (H-334 M1). A check running on a linked
//! computer holds that computer's heavy slot until its result comes back,
//! and a daemon restart there (every install does one) loses the run and
//! its result. Each time a link comes up, the board's home asks that
//! computer which of the jobs routed to it before the link it still holds
//! (`check_jobs_running`); every other one is an `error` at once, retried
//! once like any, so the slot frees without waiting out the deadline. A
//! computer too old to answer is left to `reconcile`'s deadline.

use std::collections::HashSet;
use std::sync::Arc;

use bus::Peer;
use serde_json::{json, Value};

use crate::app::AppState;
use crate::db::check_jobs::CheckJob;
use crate::prs::{check_jobs, check_rerun};

/// The peer request: which of these jobs do you still run?
pub const RUNNING: &str = "check_jobs_running";

/// The most job ids one question carries.
const MOST: usize = 256;

/// A link to `peer_id` came up: errors the jobs it no longer runs.
pub async fn link_up(app: Arc<AppState>, peer_id: String) {
    if let Err(error) = settle(&app, &peer_id).await {
        tracing::warn!(peer_id, %error, "lost check jobs not checked");
    }
}

async fn settle(app: &Arc<AppState>, peer_id: &str) -> anyhow::Result<()> {
    let Some(peer) = app.db.get_peer(peer_id)? else {
        return Ok(());
    };
    let Some(since) = app.peers.online_since(peer_id) else {
        return Ok(());
    };
    // Only jobs sent before this link: one routed over it is still starting.
    let mut jobs: Vec<CheckJob> = Vec::new();
    for project in check_jobs::home_projects(app)? {
        jobs.extend(
            app.db
                .board_read(|t| t.open_jobs(&project))?
                .into_iter()
                .filter(|j| j.machine == peer.name && j.created_at < since),
        );
    }
    jobs.truncate(MOST);
    if jobs.is_empty() {
        return Ok(());
    }
    let ids: Vec<&str> = jobs.iter().map(|j| j.id.as_str()).collect();
    let answer = match app
        .peers
        .request(peer_id, json!({"type": RUNNING, "jobs": ids}))
        .await
    {
        Ok(answer) => answer,
        Err(error) => {
            // An older daemon doesn't know the question: the deadline stays.
            tracing::info!(peer = %peer.name, %error, "check_jobs_running not answered");
            return Ok(());
        }
    };
    let held: HashSet<&str> = answer["running"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    let why = format!("{} restarted while it ran", peer.name);
    let mut lost = 0;
    for job in jobs.iter().filter(|j| !held.contains(j.id.as_str())) {
        // A result that came in while we asked has ended the job already.
        let still_open = app
            .db
            .board_read(|t| t.check_job(&job.id))?
            .is_some_and(|j| j.ended_at.is_none());
        if still_open {
            check_rerun::lose(app, job, &why)?;
            lost += 1;
        }
    }
    if lost > 0 {
        tracing::info!(peer = %peer.name, lost, "check jobs lost in a restart ended");
        check_jobs::nudge();
    }
    Ok(())
}

/// The board's home asks which of `jobs` this computer still runs: those
/// it started for that home and hasn't sent a result for.
pub fn serve(_app: &AppState, _peer: &Peer, frame: &Value) -> anyhow::Result<Value> {
    let running: Vec<&str> = frame["jobs"]
        .as_array()
        .into_iter()
        .flatten()
        .take(MOST)
        .filter_map(Value::as_str)
        .filter(|job| check_jobs::is_running_here(job))
        .collect();
    Ok(json!({ "running": running }))
}
