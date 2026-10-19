//! The card each bot's turn is about (0.19.0), so `bot_state` can say "New
//! Bot started on P2-001" rather than only "working". Known two ways: a
//! delivery that named a card started the turn (a `[card …]` notice, a task
//! delegated for an item, the owner's answer to a question on it), or the bot
//! changed a card during it. A turn that is neither, a chat with the owner
//! say, names no card.

use std::collections::HashMap;
use std::sync::Mutex;

use bus::BotState;

#[derive(Default)]
struct Turn {
    /// The card the last delivery named, for the turn it starts.
    delivered: Option<String>,
    /// That delivery was the owner's answer to a question (0.22.0).
    delivered_answer: bool,
    /// The card the running turn is about, from its delivery.
    current: Option<String>,
    /// The running turn began on the owner's answer.
    answering: bool,
    /// The last card the bot changed during the running turn.
    touched: Option<String>,
}

#[derive(Default)]
pub struct WorkingOn {
    turns: Mutex<HashMap<String, Turn>>,
}

/// A state the bot is in while a turn runs: a prompt or approval wait inside
/// a turn doesn't end it.
pub(super) fn in_turn(state: BotState) -> bool {
    matches!(
        state,
        BotState::Working | BotState::WaitingForApproval | BotState::WaitingForUser
    )
}

impl WorkingOn {
    fn with<T>(&self, bot_id: &str, f: impl FnOnce(&mut Turn) -> T) -> T {
        let mut turns = self.turns.lock().unwrap_or_else(|e| e.into_inner());
        f(turns.entry(bot_id.to_string()).or_default())
    }

    /// A delivery naming `item_id` is about to reach the bot. Recorded before
    /// it is handed over, so the turn it starts finds it.
    pub fn delivered(&self, bot_id: &str, item_id: &str) {
        self.with(bot_id, |t| {
            t.delivered = Some(item_id.to_string());
            t.delivered_answer = false;
        });
    }

    /// The owner's answer to a question on `item_id` is about to reach the
    /// bot (0.22.0), so the turn it starts says it got the answer.
    pub fn answer_delivered(&self, bot_id: &str, item_id: &str) {
        self.with(bot_id, |t| {
            t.delivered = Some(item_id.to_string());
            t.delivered_answer = true;
        });
    }

    /// Whether the running turn began on the owner's answer.
    pub fn answering(&self, bot_id: &str) -> bool {
        self.with(bot_id, |t| t.answering)
    }

    /// The bot changed `item_id`.
    pub fn touched(&self, bot_id: &str, item_id: &str) {
        self.with(bot_id, |t| t.touched = Some(item_id.to_string()));
    }

    /// The bot went from `from` to `to`: the card the turn starting, going on
    /// or ending there is about, if one is known.
    pub fn on_state(&self, bot_id: &str, from: BotState, to: BotState) -> Option<String> {
        self.with(bot_id, |t| match (in_turn(from), in_turn(to)) {
            (false, true) => {
                t.current = t.delivered.take();
                t.answering = std::mem::take(&mut t.delivered_answer);
                t.touched = None;
                t.current.clone()
            }
            (true, true) => t.current.clone().or_else(|| t.touched.clone()),
            (true, false) => {
                let item = t.current.take().or_else(|| t.touched.take());
                t.touched = None;
                t.answering = false;
                item
            }
            (false, false) => None,
        })
    }

    /// The bot's runtime went away mid-turn: nothing carries over.
    pub fn forget(&self, bot_id: &str) {
        let mut turns = self.turns.lock().unwrap_or_else(|e| e.into_inner());
        turns.remove(bot_id);
    }
}

/// The card a delivered message names: `[card P2-001] …` at its start, the
/// way the daemon's own card notices begin.
pub fn card_named(body: &str) -> Option<&str> {
    let rest = body.trim_start().strip_prefix("[card ")?;
    let (id, _) = rest.split_once(']')?;
    let id = id.trim();
    (!id.is_empty() && !id.contains(char::is_whitespace)).then_some(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use BotState::{Ready, WaitingForApproval, Working};

    #[test]
    fn a_turn_names_the_card_its_delivery_named() {
        let w = WorkingOn::default();
        w.delivered("b", "P-1");
        assert_eq!(w.on_state("b", Ready, Working).as_deref(), Some("P-1"));
        assert_eq!(
            w.on_state("b", Working, WaitingForApproval).as_deref(),
            Some("P-1")
        );
        assert_eq!(
            w.on_state("b", WaitingForApproval, Ready).as_deref(),
            Some("P-1")
        );
        // The next turn, a chat, names none.
        assert_eq!(w.on_state("b", Ready, Working), None);
        assert_eq!(w.on_state("b", Working, Ready), None);
    }

    #[test]
    fn a_turn_an_answer_starts_says_so_until_it_ends() {
        let w = WorkingOn::default();
        w.answer_delivered("b", "P-3");
        assert!(!w.answering("b"));
        assert_eq!(w.on_state("b", Ready, Working).as_deref(), Some("P-3"));
        assert!(w.answering("b"));
        assert_eq!(w.on_state("b", Working, Ready).as_deref(), Some("P-3"));
        assert!(!w.answering("b"));
        // A card notice afterwards is no answer.
        w.answer_delivered("b", "P-3");
        w.delivered("b", "P-4");
        assert_eq!(w.on_state("b", Ready, Working).as_deref(), Some("P-4"));
        assert!(!w.answering("b"));
    }

    #[test]
    fn a_turn_ends_on_the_card_the_bot_changed() {
        let w = WorkingOn::default();
        assert_eq!(w.on_state("b", Ready, Working), None);
        w.touched("b", "P-2");
        assert_eq!(w.on_state("b", Working, Ready).as_deref(), Some("P-2"));
        assert_eq!(w.on_state("b", Ready, Working), None);
    }

    #[test]
    fn a_card_is_named_at_the_start_of_a_message() {
        assert_eq!(
            card_named("[card P2-001] The owner changed it"),
            Some("P2-001")
        );
        assert_eq!(card_named("  [card X-9]"), Some("X-9"));
        assert_eq!(card_named("see [card P2-001]"), None);
        assert_eq!(card_named("[card ]"), None);
        assert_eq!(card_named("[card two words]"), None);
    }
}
