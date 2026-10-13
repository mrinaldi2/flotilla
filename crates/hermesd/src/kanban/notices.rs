//! Notices to an item's assignee when the owner edits or moves it (0.19.0
//! K4b). A bot is told once per item in any 60 s: the edits that follow in
//! that window are folded into the first notice, which already asks it to
//! read the item again. The note budget caps the rest, as for every note.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use bus::MessageKind;

use super::model::{Item, ItemState};
use crate::app::AppState;
use crate::messaging::{self, Dm};

/// How long the notices for one bot and item fold into one.
const WINDOW: Duration = Duration::from_secs(60);

fn recent() -> &'static Mutex<HashMap<(String, String), Instant>> {
    static RECENT: OnceLock<Mutex<HashMap<(String, String), Instant>>> = OnceLock::new();
    RECENT.get_or_init(Mutex::default)
}

/// Whether `bot` was told about `item` within the window; records it now
/// when not.
fn folded(bot_id: &str, item_id: &str, now: Instant) -> bool {
    let mut seen = recent().lock().unwrap_or_else(|e| e.into_inner());
    seen.retain(|_, at| now.duration_since(*at) < WINDOW);
    let key = (bot_id.to_string(), item_id.to_string());
    if seen.contains_key(&key) {
        return true;
    }
    seen.insert(key, now);
    false
}

/// Tell a live item's assignee that the owner changed it: `what` says how
/// ("edited it", "moved it to review"). A failed delivery is logged;
/// the change itself stands.
pub fn tell_assignee(app: &AppState, item: &Item, what: &str) {
    let Some(bot_id) = item.assignee.as_deref() else {
        return;
    };
    if item.state != ItemState::Live || folded(bot_id, &item.id, Instant::now()) {
        return;
    }
    if !matches!(app.db.get_live_bot(bot_id), Ok(Some(_))) {
        return;
    }
    let text = format!(
        "[card {id}] The owner changed {id}, which is assigned to you: {what}. Read it again \
         with item_get before you go on.",
        id = item.id
    );
    let sender = messaging::user_sender();
    let dm = Dm::new(bot_id, &sender, MessageKind::Note, &text);
    if let Err(e) = messaging::send_dm(&app.db, &app.events, dm) {
        tracing::warn!(bot_id, item_id = %item.id, error = %e, "assignee notice not delivered");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notices_for_one_item_fold_for_a_minute() {
        let t0 = Instant::now();
        assert!(!folded("b-fold", "H-1", t0));
        assert!(folded("b-fold", "H-1", t0 + Duration::from_secs(30)));
        assert!(!folded("b-fold", "H-2", t0 + Duration::from_secs(30)));
        assert!(!folded("b-fold", "H-1", t0 + Duration::from_secs(61)));
    }
}
