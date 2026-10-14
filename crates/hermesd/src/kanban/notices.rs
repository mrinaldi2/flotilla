//! Notices to an item's assignee when the owner or another bot assigns,
//! edits or moves it (0.19.0 K4b, card flow). The text comes from the
//! card's board (`notice_text`): which card, where it is, and what is
//! expected. An assignment or an unassignment is always told; an edit, or
//! a move, is told once per item in any 60 s, and the edits (or moves) that
//! follow in that window fold into it, which already asks the bot to read
//! the item again. The edits right after an assignment fold into it.
//! The note budget caps the rest, as for every note.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use bus::MessageKind;

use super::model::{Item, ItemState};
use super::notice_text::{self, Card, Notice, Plan};
use crate::actor::Actor;
use crate::app::AppState;
use crate::messaging::{self, Dm};

/// How long the notices for one bot and item fold into one.
const WINDOW: Duration = Duration::from_secs(60);

type Key = (String, String, &'static str);

fn recent() -> &'static Mutex<HashMap<Key, Instant>> {
    static RECENT: OnceLock<Mutex<HashMap<Key, Instant>>> = OnceLock::new();
    RECENT.get_or_init(Mutex::default)
}

/// Whether `bot` was told of the same kind of change to `item` within the
/// window; records it now when not. Edits fold into edits and into the
/// assignment before them, moves into moves; an assignment (`always`) is
/// never folded.
fn folded(bot_id: &str, item_id: &str, kind: &'static str, now: Instant, always: bool) -> bool {
    let mut seen = recent().lock().unwrap_or_else(|e| e.into_inner());
    seen.retain(|_, at| now.duration_since(*at) < WINDOW);
    let key = (bot_id.to_string(), item_id.to_string(), kind);
    if !always && seen.contains_key(&key) {
        return true;
    }
    seen.insert(key, now);
    false
}

/// Tell a live item's assignee what `by` did to it. The assignee's own
/// changes and the daemon's tell nobody. A failed delivery is logged; the
/// change itself stands.
pub fn tell_assignee(app: &AppState, item: &Item, by: &Actor<'_>, notice: Notice<'_>) {
    if let Some(bot_id) = item.assignee.as_deref() {
        tell(app, item, bot_id, by, notice);
    }
}

/// After an assignment: the new assignee hears the card is theirs, the
/// previous one that it no longer is.
pub fn tell_reassigned(app: &AppState, before: Option<&str>, item: &Item, by: &Actor<'_>) {
    if before == item.assignee.as_deref() {
        return;
    }
    if let Some(previous) = before {
        tell(app, item, previous, by, Notice::Unassigned);
    }
    tell_assignee(app, item, by, Notice::Assigned);
}

fn tell(app: &AppState, item: &Item, bot_id: &str, by: &Actor<'_>, notice: Notice<'_>) {
    if item.state != ItemState::Live || matches!(by, Actor::Daemon) || by.bot_id() == Some(bot_id) {
        return;
    }
    let (kind, always) = match notice {
        Notice::Assigned | Notice::Unassigned => ("edit", true),
        Notice::Moved => ("move", false),
        Notice::Changed(_) => ("edit", false),
    };
    if folded(bot_id, &item.id, kind, Instant::now(), always) {
        return;
    }
    if !matches!(app.db.get_live_bot(bot_id), Ok(Some(_))) {
        return;
    }
    let (sender, by_name) = match by.bot_id() {
        Some(id) => match app.db.get_bot(id) {
            Ok(Some(bot)) => (crate::mcp::bot_sender(&bot), bot.name),
            _ => return,
        },
        None => (messaging::user_sender(), "The owner".to_string()),
    };
    let text = match compose(app, item, bot_id, &by_name, notice) {
        Ok(Some(text)) => text,
        Ok(None) => return,
        Err(e) => {
            tracing::warn!(bot_id, item_id = %item.id, error = %e, "assignee notice not composed");
            return;
        }
    };
    let dm = Dm::new(bot_id, &sender, MessageKind::Note, &text);
    if let Err(e) = messaging::send_dm(&app.db, &app.events, dm) {
        tracing::warn!(bot_id, item_id = %item.id, error = %e, "assignee notice not delivered");
    }
}

/// The notice's text, from the card's board as it is now.
fn compose(
    app: &AppState,
    item: &Item,
    bot_id: &str,
    by: &str,
    notice: Notice<'_>,
) -> anyhow::Result<Option<String>> {
    app.db.board_read(|t| {
        let Some(project_id) = t.item_project(&item.id)? else {
            return Ok(None);
        };
        let columns: Vec<_> = t
            .columns(&project_id)?
            .into_iter()
            .filter(|c| item.board_id.is_empty() || c.board_id == item.board_id)
            .collect();
        let Some(column) = columns.iter().find(|c| c.key == item.column_key) else {
            return Ok(None);
        };
        let board = t.board(&item.board_id)?;
        let author = format!("bot:{bot_id}");
        let comments = t.item_comments(&item.id)?;
        let last_comment = comments
            .iter()
            .rev()
            .find(|c| c.author == author)
            .map(|c| c.body.as_str());
        let card = Card {
            item,
            by,
            column,
            plan: Plan::of(board.as_ref(), &columns),
            last_comment,
        };
        Ok(Some(notice_text::text(&card, notice)))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notices_for_one_item_fold_for_a_minute() {
        let t0 = Instant::now();
        assert!(!folded("b-fold", "H-1", "edit", t0, false));
        assert!(folded(
            "b-fold",
            "H-1",
            "edit",
            t0 + Duration::from_secs(30),
            false
        ));
        assert!(!folded(
            "b-fold",
            "H-2",
            "edit",
            t0 + Duration::from_secs(30),
            false
        ));
        assert!(!folded(
            "b-fold",
            "H-1",
            "edit",
            t0 + Duration::from_secs(61),
            false
        ));
    }

    #[test]
    fn an_assignment_is_never_folded_and_a_move_is_its_own_news() {
        let t0 = Instant::now();
        assert!(!folded("b-assign", "H-1", "edit", t0, false));
        assert!(!folded(
            "b-assign",
            "H-1",
            "edit",
            t0 + Duration::from_secs(5),
            true
        ));
        assert!(folded(
            "b-assign",
            "H-1",
            "edit",
            t0 + Duration::from_secs(10),
            false
        ));
        assert!(!folded(
            "b-assign",
            "H-1",
            "move",
            t0 + Duration::from_secs(10),
            false
        ));
        assert!(folded(
            "b-assign",
            "H-1",
            "move",
            t0 + Duration::from_secs(20),
            false
        ));
    }
}
