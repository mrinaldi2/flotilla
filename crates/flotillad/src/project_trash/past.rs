//! Cleaning up the past: what projects deleted before 0.19 left on disk
//! (their folders until retention, their bots' session history and git
//! caches forever), found the same way as for a delete and moved to the
//! trash on the owner's word. `flotillad cleanup deleted-projects` and
//! Settings → Storage both use it.

use serde_json::{json, Value};

use super::{collect, run, Leftovers};
use crate::app::AppState;

/// Every deleted project that still left something here.
pub fn leftovers(app: &AppState) -> Vec<Leftovers> {
    app.db
        .list_deleted_projects()
        .unwrap_or_default()
        .iter()
        .map(|project| collect::collect(app, project))
        .filter(|found| !found.is_empty())
        .collect()
}

/// The preview: each project's groups, the total, where it goes, and the
/// linked computers that haven't confirmed their half.
pub fn view(app: &AppState) -> Value {
    let found = leftovers(app);
    let total: u64 = found.iter().map(Leftovers::total_bytes).sum();
    let waiting: Vec<Value> = ["pending", "unconfirmed"]
        .iter()
        .flat_map(|state| app.db.trash_orders(state, None).unwrap_or_default())
        .map(|order| {
            let name = app
                .db
                .get_peer(&order.peer_id)
                .ok()
                .flatten()
                .map_or_else(|| order.peer_id.clone(), |p| p.name);
            json!({
                "project_name": order.project_name, "peer_name": name,
                "state": order.state, "attempts": order.attempts,
            })
        })
        .collect();
    json!({
        "projects": found.iter().map(Leftovers::view).collect::<Vec<_>>(),
        "total_bytes": total,
        "bin": crate::trash::bin_name(crate::trash::first_bin(&app.cfg)),
        "linked": waiting,
    })
}

/// Moves everything [`leftovers`] finds now to the trash.
pub fn clean(app: &AppState) -> run::Report {
    let mut report = run::Report::default();
    for found in leftovers(app) {
        report.absorb(run::trash(app, &found));
    }
    report
}
