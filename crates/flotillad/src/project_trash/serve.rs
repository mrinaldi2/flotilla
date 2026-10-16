//! A linked computer deleted its project: trash this computer's half.
//!
//! The project here must be the one linked with it, by the link or, when
//! the unlink that follows the delete got here first, by the stand-ins of
//! its bots that the link left archived here. Then:
//!
//! - the bots it had created here for the project (their creator is one of
//!   its bots' stand-ins) are deleted, and their folders and history go to
//!   the trash;
//! - when that leaves the project as only a mirror of the deleted one (no
//!   bot of its own, no board here, no link to another computer), the
//!   project is deleted here too and everything it left goes to the trash;
//! - otherwise the project stays: its own team still works in it.
//!
//! Nothing the frame names is taken as a path: everything is found and
//! confined here, as for a delete on this computer.

use std::sync::Arc;

use bus::{Bot, Peer, Project};
use serde_json::{json, Value};

use super::{collect, peer::DONE, run};
use crate::app::AppState;
use crate::db::Actor;
use crate::events::Push;

/// The `project_trash` event from `peer`. Runs on the link's ordered event
/// feed: deciding and archiving happen here, before a later frame of the
/// same link (the unlink) is applied; the moves happen on a thread of
/// their own, and the answer follows when they are done.
pub fn receive(app: &Arc<AppState>, peer: &Peer, frame: &Value) {
    if app.cfg.ignore_trash_asks {
        return;
    }
    let order_id = frame
        .get("order_id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let answer = move |outcome: &str, report: &run::Report, summary: String| {
        json!({
            "type": DONE, "order_id": order_id, "outcome": outcome,
            "moved_bytes": report.moved_bytes(), "skipped": report.skipped.len(),
            "summary": summary,
        })
    };
    let Some(project) = linked_project(app, peer, frame) else {
        let none = run::Report::default();
        let text = "No project here is linked with it; nothing was moved.".to_string();
        app.peers
            .notify(&peer.id, answer("not_linked", &none, text));
        return;
    };
    let decided = decide(app, peer, &project);
    let (app, peer) = (app.clone(), peer.clone());
    super::in_background("project_trash_serve", move || {
        let (outcome, leftovers, kept) = decided;
        let report = run::trash(&app, &leftovers);
        let name = crate::db::Db::display_project_name(&project);
        let mut summary = format!("{name}: {}", report.sentence());
        if let Some(why) = &kept {
            summary.push_str(&format!(" The project stays here: {why}."));
        }
        app.events.push(Push::notice(
            "info",
            format!("{name} deleted on {}", peer.name),
            summary.clone(),
        ));
        app.peers
            .notify(&peer.id, answer(outcome, &report, summary));
    });
}

/// The project here the frame names, when it is (or was) linked with `peer`.
fn linked_project(app: &AppState, peer: &Peer, frame: &Value) -> Option<Project> {
    let id = frame.get("project_id").and_then(Value::as_str)?;
    let from = frame.get("from_project_id").and_then(Value::as_str)?;
    let project = app.db.get_project(id).ok()??;
    let by_link = app
        .db
        .project_link_by_remote(&peer.id, from)
        .ok()
        .flatten()
        .is_some_and(|link| link.project_id == project.id);
    let by_stand_ins = !stand_ins(app, peer, &project).is_empty();
    (by_link || by_stand_ins).then_some(project)
}

/// Its bots' stand-ins here, archived ones included.
fn stand_ins(app: &AppState, peer: &Peer, project: &Project) -> Vec<Bot> {
    app.db
        .list_bots_with_archived(Some(&project.id))
        .unwrap_or_default()
        .into_iter()
        .filter(|b| b.peer_id.as_deref() == Some(peer.id.as_str()))
        .collect()
}

type Decided = (&'static str, super::Leftovers, Option<String>);

/// Deletes what the linked project's bots created here, and the project
/// when only its mirror is left; returns what to trash.
fn decide(app: &Arc<AppState>, peer: &Peer, project: &Project) -> Decided {
    let theirs: Vec<String> = stand_ins(app, peer, project)
        .into_iter()
        .map(|b| b.id)
        .collect();
    let children: Vec<Bot> = app
        .db
        .list_bots_with_archived(Some(&project.id))
        .unwrap_or_default()
        .into_iter()
        .filter(|b| !b.is_linked())
        .filter(|b| {
            b.created_by_bot_id
                .as_ref()
                .is_some_and(|c| theirs.contains(c))
        })
        .collect();
    let reason = format!("its project was deleted on {}", peer.name);
    for bot in children.iter().filter(|b| b.deleted_at.is_none()) {
        if let Err(e) = crate::botmgmt::archive_bot(app, bot, &Actor::User, Some(&reason)) {
            tracing::warn!(bot_id = %bot.id, error = %e, "a linked project's bot not deleted");
        }
    }
    if project.deleted_at.is_some() {
        return ("trashed", collect::collect(app, project), None);
    }
    match kept_because(app, peer, project) {
        Some(why) => (
            "kept",
            collect::collect_bots(app, project, &children),
            Some(why),
        ),
        None => {
            if let Err(e) = crate::projectmgmt::archive_project(app, project, &Actor::User) {
                tracing::warn!(project_id = %project.id, error = %e, "linked project not deleted");
                return (
                    "kept",
                    collect::collect_bots(app, project, &children),
                    Some(e.to_string()),
                );
            }
            let archived = app
                .db
                .get_project(&project.id)
                .ok()
                .flatten()
                .unwrap_or_else(|| project.clone());
            ("trashed", collect::collect(app, &archived), None)
        }
    }
}

/// Why the project here is more than the deleted one's mirror, if it is.
fn kept_because(app: &AppState, peer: &Peer, project: &Project) -> Option<String> {
    let own = app
        .db
        .list_bots(Some(&project.id))
        .unwrap_or_default()
        .into_iter()
        .filter(|b| !b.is_linked())
        .count();
    if own > 0 {
        return Some(format!(
            "it still has {own} bot{} of its own",
            if own == 1 { "" } else { "s" }
        ));
    }
    if matches!(app.db.board_settings(&project.id), Ok(Some(_))) {
        return Some("its board lives on this computer".to_string());
    }
    let other = app
        .db
        .project_links(&project.id)
        .unwrap_or_default()
        .into_iter()
        .any(|l| l.peer_id != peer.id);
    other.then(|| "it is linked with another computer too".to_string())
}
