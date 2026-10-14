//! What happens on the boards, told to the owner's apps (0.19.0): who moved
//! or assigned which card, a bot's edits to a draft, and moves the guards
//! refused. Every connection gets a `board_activity` push, whatever boards it
//! watches, so the desktop can toast it anywhere and a phone can raise a
//! notice; the connection that asked for a change is told it was its own.
//!
//! Changes come off the board feed with the history they recorded
//! (`feed_by`); a refusal writes nothing, so its handler announces it here.

use std::sync::Arc;

use bus::contract::home::{BoardActivity, BoardActivityKind as Kind};
use tokio::sync::broadcast::error::RecvError;

use super::feed::{BoardChange, ChangeKind};
use super::feed_by::DAEMON;
use super::model::{ItemEvent, ItemEventKind, Unmet};
use crate::actor::Actor;
use crate::app::AppState;
use crate::events::Push;

/// Announce every board change worth telling, for as long as the daemon runs.
pub fn spawn(app: Arc<AppState>) {
    let mut rx = app.board.subscribe();
    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(change) => announce(&app, &change),
                // Missed ones are history by now; a toast late is no toast.
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => break,
            }
        }
    });
}

fn announce(app: &AppState, change: &BoardChange) {
    for event in &change.events {
        if let Some(bot_id) = event.actor.strip_prefix("bot:") {
            app.supervisor.working_on().touched(bot_id, &change.item_id);
        }
    }
    for activity in activities(change) {
        push(app, activity, change.origin);
    }
}

/// The activities one change makes: its newest move, its assignments, and
/// a bot's edits to a draft folded into one.
fn activities(change: &BoardChange) -> Vec<BoardActivity> {
    let base = BoardActivity {
        project_id: change.project_id.clone(),
        item_id: change.item_id.clone(),
        item_title: change
            .card
            .as_ref()
            .map(|c| c.title.clone())
            .unwrap_or_default(),
        ..Default::default()
    };
    let mut out = Vec::new();
    let events = &change.events;
    let moved = events.iter().rev().find(|e| e.kind == ItemEventKind::Moved);
    match moved {
        Some(e) => out.push(BoardActivity {
            kind: Kind::Moved as i32,
            // A fact's move is the daemon's, whoever reported the fact.
            actor: match change.actor.as_deref() {
                Some(DAEMON) => DAEMON.to_string(),
                _ => e.actor.clone(),
            },
            from_column: e.from.clone().unwrap_or_default(),
            to_column: e.to.clone().unwrap_or_default(),
            note: e.note.clone().unwrap_or_default(),
            ..base.clone()
        }),
        // Relayed from a board's home, which sends no history.
        None if change.kind == ChangeKind::ItemMoved && events.is_empty() => {
            out.push(BoardActivity {
                kind: Kind::Moved as i32,
                actor: change.actor.clone().unwrap_or_default(),
                from_column: change.from_column.clone().unwrap_or_default(),
                to_column: change
                    .card
                    .as_ref()
                    .map(|c| c.column_key.clone())
                    .unwrap_or_default(),
                note: change.note.clone().unwrap_or_default(),
                ..base.clone()
            });
        }
        None => {}
    }
    for e in events.iter().filter(|e| e.kind == ItemEventKind::Assigned) {
        let assignee = e.to.clone().filter(|to| !to.is_empty());
        out.push(BoardActivity {
            kind: if assignee.is_some() {
                Kind::Assigned
            } else {
                Kind::Unassigned
            } as i32,
            actor: e.actor.clone(),
            assignee: assignee.or_else(|| e.from.clone()).unwrap_or_default(),
            ..base.clone()
        });
    }
    let draft =
        change.kind == ChangeKind::DraftChanged || change.card.as_ref().is_some_and(|c| c.draft);
    if let Some(edit) = draft.then(|| draft_edit(events)).flatten() {
        out.push(BoardActivity {
            kind: Kind::DraftUpdated as i32,
            ..edit.with(base)
        });
    }
    out
}

struct DraftEdit {
    actor: String,
    fields: Vec<String>,
}

impl DraftEdit {
    fn with(self, base: BoardActivity) -> BoardActivity {
        BoardActivity {
            actor: self.actor,
            note: self.fields.join(", "),
            ..base
        }
    }
}

/// A bot's edits to a draft: the owner sees their own, and an invitation
/// isn't an edit.
fn draft_edit(events: &[ItemEvent]) -> Option<DraftEdit> {
    let edits: Vec<&ItemEvent> = events
        .iter()
        .filter(|e| e.kind == ItemEventKind::Edited && e.actor.starts_with("bot:"))
        .filter(|e| e.field.as_deref() != Some("invited"))
        .collect();
    let last = edits.last()?;
    let mut fields: Vec<String> = Vec::new();
    for field in edits.iter().filter_map(|e| e.field.as_deref()) {
        let field = field.replace('_', " ");
        if !fields.contains(&field) {
            fields.push(field);
        }
    }
    Some(DraftEdit {
        actor: last.actor.clone(),
        fields,
    })
}

/// A move the guards refused, for the owner's apps: the bots' especially,
/// which the owner otherwise never sees. Nothing was written.
pub fn refused(app: &AppState, item_id: &str, to: &str, actor: &Actor<'_>, unmet: &[Unmet]) {
    let Ok(Some(item)) = app.db.get_item(item_id) else {
        return;
    };
    let Ok(Some(project_id)) = app.db.item_project(item_id) else {
        return;
    };
    let activity = BoardActivity {
        project_id,
        item_id: item.id.clone(),
        kind: Kind::MoveRefused as i32,
        actor: actor.as_stored(),
        item_title: item.title.clone(),
        from_column: item.column_key.clone(),
        to_column: to.to_string(),
        unmet: unmet.iter().map(|u| u.text.clone()).collect(),
        ..Default::default()
    };
    push(app, activity, super::feed_by::origin());
}

fn push(app: &AppState, mut activity: BoardActivity, origin: Option<u64>) {
    super::activity_text::fill(app, &mut activity);
    app.events.push(Push::BoardActivity {
        activity: Box::new(activity),
        origin,
    });
}

#[cfg(test)]
#[path = "activity_tests.rs"]
mod tests;
