//! Telling linked computers to trash their half of a deleted project.
//!
//! Each computer the project was linked with gets the event
//! `project_trash {order_id, project_id, from_project_id, project_name}`,
//! `project_id` being its own project. The ask is written to the database
//! before the link is dropped, sent at once over a live link, and again
//! each time the link comes back (before anything else that link sends,
//! so it lands while the computer still knows the link), and every
//! `trash.peer_resend_minutes` while it doesn't answer. It answers with
//! the event `project_trash_done {order_id, outcome, moved_bytes, skipped,
//! summary}`.
//!
//! A computer from before 0.19 ignores an event it doesn't know, so after
//! `trash.peer_attempts` asks on a live link without an answer the ask ends
//! `unconfirmed`, and the owner is told how many computers didn't confirm.

use std::collections::BTreeMap;
use std::sync::Arc;

use bus::Project;
use chrono::Utc;
use serde_json::{json, Value};

use crate::app::AppState;
use crate::db::TrashOrder;
use crate::events::Push;

/// The ask.
pub const ORDER: &str = "project_trash";
/// The answer.
pub const DONE: &str = "project_trash_done";
/// How often unanswered asks are looked at.
const SWEEP_SECS: u64 = 60;

/// The linked computers a delete of `project` would tell, for the preview.
pub fn peers_view(app: &AppState, project: &Project) -> Vec<Value> {
    app.db
        .project_links(&project.id)
        .unwrap_or_default()
        .into_iter()
        .map(|link| {
            let name = app
                .db
                .get_peer(&link.peer_id)
                .ok()
                .flatten()
                .map_or_else(|| link.peer_id.clone(), |p| p.name);
            json!({
                "peer_id": link.peer_id,
                "name": name,
                "online": app.peers.is_online(&link.peer_id),
            })
        })
        .collect()
}

/// Writes an ask for every computer `project` is linked with and sends it
/// to those online. Called before the project is archived, which drops
/// the links.
pub fn record(app: &AppState, project: &Project) -> Vec<TrashOrder> {
    let mut orders = Vec::new();
    // Scratch mode links no computer and must not leave asks behind (H-171).
    if app.cfg.scratch {
        return orders;
    }
    for link in app.db.project_links(&project.id).unwrap_or_default() {
        match app
            .db
            .insert_trash_order(project, &link.peer_id, &link.remote_project_id)
        {
            Ok(order) => {
                send(app, &order);
                orders.push(order);
            }
            Err(e) => {
                tracing::warn!(peer_id = %link.peer_id, error = %e, "trash ask not recorded");
            }
        }
    }
    orders
}

fn frame(order: &TrashOrder) -> Value {
    json!({
        "type": ORDER,
        "order_id": order.id,
        "project_id": order.remote_project_id,
        "from_project_id": order.project_id,
        "project_name": order.project_name,
    })
}

/// Sends the ask when its computer is online. True when it went.
fn send(app: &AppState, order: &TrashOrder) -> bool {
    if !app.peers.is_online(&order.peer_id) {
        return false;
    }
    app.peers.notify(&order.peer_id, frame(order));
    if let Err(e) = app.db.trash_order_sent(&order.id) {
        tracing::warn!(order = %order.id, error = %e, "trash ask not marked sent");
    }
    tracing::info!(peer_id = %order.peer_id, project = %order.project_name, "asked to trash a linked project");
    true
}

/// The link to `peer_id` is up: every ask still waiting for it goes first.
pub fn link_up(app: &AppState, peer_id: &str) {
    for order in app
        .db
        .trash_orders("pending", Some(peer_id))
        .unwrap_or_default()
    {
        send(app, &order);
    }
}

/// Asks again those not answered for a while, and ends as unconfirmed the
/// ones asked often enough. Returns how many ended so.
pub fn sweep(app: &AppState) -> usize {
    let resend = chrono::Duration::minutes(app.cfg.trash.peer_resend_minutes.max(0));
    let now = Utc::now();
    let mut unconfirmed: BTreeMap<String, usize> = BTreeMap::new();
    for order in app.db.trash_orders("pending", None).unwrap_or_default() {
        if !app.peers.is_online(&order.peer_id) {
            continue;
        }
        let due = order.sent_at.is_none_or(|sent| now - sent >= resend);
        if !due {
            continue;
        }
        if order.attempts >= app.cfg.trash.peer_attempts.max(1) {
            if app
                .db
                .end_trash_order(&order.id, "unconfirmed", None, 0)
                .unwrap_or(false)
            {
                *unconfirmed.entry(order.project_name.clone()).or_default() += 1;
            }
            continue;
        }
        send(app, &order);
    }
    for (project, n) in &unconfirmed {
        let computers = if *n == 1 {
            "1 linked computer".to_string()
        } else {
            format!("{n} linked computers")
        };
        app.events.push(Push::notice(
            "warn",
            "Linked computer didn't confirm",
            format!(
                "{computers} didn't confirm moving its copy of {project} to the trash. It may \
                 run a version from before 0.19; Clean up deleted projects there removes it."
            ),
        ));
    }
    unconfirmed.values().sum()
}

/// A computer answered an ask.
pub fn receive_done(app: &Arc<AppState>, peer_id: &str, frame: &Value) {
    let Some(order_id) = frame.get("order_id").and_then(Value::as_str) else {
        return;
    };
    let Ok(Some(order)) = app.db.trash_order(order_id) else {
        return;
    };
    if order.peer_id != peer_id {
        tracing::warn!(
            peer_id,
            order_id,
            "a trash answer from a computer not asked"
        );
        return;
    }
    let moved = frame
        .get("moved_bytes")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    match app
        .db
        .end_trash_order(order_id, "confirmed", Some(frame), moved)
    {
        Ok(true) => {}
        Ok(false) => return,
        Err(e) => {
            tracing::warn!(order_id, error = %e, "trash answer not recorded");
            return;
        }
    }
    let computer = app
        .db
        .get_peer(peer_id)
        .ok()
        .flatten()
        .map_or_else(|| "A linked computer".to_string(), |p| p.name);
    let summary = frame
        .get("summary")
        .and_then(Value::as_str)
        .unwrap_or("It cleaned up its copy.");
    app.events.push(Push::notice(
        "info",
        format!("{} cleaned up on {computer}", order.project_name),
        summary.to_string(),
    ));
}

/// Looks at unanswered asks every minute.
pub fn spawn_sweeper(app: &Arc<AppState>) {
    let app = app.clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(SWEEP_SECS));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tick.tick().await;
            let app = app.clone();
            let _ = tokio::task::spawn_blocking(move || sweep(&app)).await;
        }
    });
}
