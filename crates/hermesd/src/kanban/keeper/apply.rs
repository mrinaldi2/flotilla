//! Carrying out a card's plan: the notes to its assignee (as the daemon's,
//! `system`, a note: news about a card that is already the bot's), the
//! marks, the card's push when the board shows it differently, and the
//! owner's notice of what the keeper did. A pass sends at most so many
//! notes, and one per bot; the rest wait for the next pass, unmarked.

use std::collections::HashSet;
use std::sync::Arc;

use bus::MessageKind;

use super::gather::Project;
use super::rules::{Act, Plan, Rule, Say};
use super::text::{self, About};
use crate::app::AppState;
use crate::db::KeeperCard;
use crate::kanban::notice_text::{self, Card};
use crate::messaging::{self, Dm};

/// What one pass may still do.
pub(super) struct Budget {
    messages: usize,
    /// The bots told this pass; none when a bot may be told more than once.
    told: Option<HashSet<String>>,
    publishes: usize,
}

/// Cards one pass pushes at most; the rest are judged again next pass.
const PUBLISHES_PER_PASS: usize = 50;

impl Budget {
    /// A pass's: `messages` notes, one per bot.
    pub fn new(messages: usize) -> Self {
        Self {
            messages,
            told: Some(HashSet::new()),
            publishes: PUBLISHES_PER_PASS,
        }
    }

    /// A turn's end: up to `messages` notes, all to the bot whose turn it was.
    pub fn for_turn(messages: usize) -> Self {
        Self {
            told: None,
            ..Self::new(messages)
        }
    }

    fn may_tell(&self, bot_id: &str) -> bool {
        self.messages > 0 && self.told.as_ref().is_none_or(|t| !t.contains(bot_id))
    }

    fn told(&mut self, bot_id: &str) {
        self.messages = self.messages.saturating_sub(1);
        if let Some(told) = &mut self.told {
            told.insert(bot_id.to_string());
        }
    }
}

/// What the keeper did that the owner hears of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Done {
    Told(Rule),
    Raised(Rule),
}

/// Carry out `plan` for `card`. Returns what the owner should hear of.
pub(super) fn apply(
    app: &Arc<AppState>,
    project: &Project,
    card: &KeeperCard,
    plan: Plan,
    budget: &mut Budget,
) -> anyhow::Result<Vec<Done>> {
    if plan.publish && budget.publishes == 0 {
        return Ok(Vec::new());
    }
    let mut done = Vec::new();
    for act in plan.acts {
        match act {
            Act::Say(say, mark) => {
                let Some(bot_id) = card.assignee.as_deref() else {
                    continue;
                };
                if !budget.may_tell(bot_id) {
                    continue;
                }
                let Some(body) = compose(app, project, card, &say)? else {
                    continue;
                };
                let sender = messaging::daemon_sender();
                let dm = Dm::new(bot_id, &sender, MessageKind::Note, &body);
                if let Err(e) = messaging::send_dm(&app.db, &app.events, dm) {
                    tracing::warn!(bot_id, item_id = %card.id, error = %e, "keeper note not sent");
                    continue;
                }
                budget.told(bot_id);
                app.db.keeper_set(&mark)?;
                done.push(Done::Told(rule_of(&say)));
            }
            Act::Set(mark) => app.db.keeper_set(&mark)?,
            Act::Clear(rule) => app.db.keeper_clear(&card.id, rule.name())?,
            Act::Raised(rule) => done.push(Done::Raised(rule)),
        }
    }
    if plan.publish {
        budget.publishes -= 1;
        crate::kanban::stuck::publish_card(app, &project.id, &card.id);
    }
    Ok(done)
}

fn rule_of(say: &Say) -> Rule {
    match say {
        Say::Nudge(_) => Rule::SilentNudged,
        Say::Reminder => Rule::NotStarted,
        Say::AskToMove => Rule::NotMoved,
    }
}

/// The note's text, from the card's board as it is now.
fn compose(
    app: &AppState,
    project: &Project,
    card: &KeeperCard,
    say: &Say,
) -> anyhow::Result<Option<String>> {
    let Some(column) = project.column_of(card) else {
        return Ok(None);
    };
    let board = project.board_of(card);
    let about = About {
        id: &card.id,
        title: &card.title,
        column,
        plan: board.plan.as_ref(),
    };
    Ok(Some(match say {
        Say::Nudge(quiet) => text::nudge(&about, *quiet),
        Say::AskToMove => text::ask_to_move(&about),
        Say::Reminder => {
            let Some(item) = app.db.get_item(&card.id)? else {
                return Ok(None);
            };
            let notice = Card {
                item: &item,
                by: "",
                column,
                plan: notice_text::Plan::of(board.board.as_ref(), &project.columns),
                last_comment: None,
            };
            text::reminder(&about, &notice_text::assigned_next(&notice))
        }
    }))
}
