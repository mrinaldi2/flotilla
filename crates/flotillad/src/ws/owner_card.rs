//! A card on the owner's message (H-128 §3, D5): `send_user_message` may
//! name the card the work belongs to. It must be on the bot's project's
//! board and still open. The message carries it as a `[card H-nnn]` lead,
//! which the bot's prompt says to work on, and the card gets an "Owner
//! asked" comment where its board lives here. A draft, which only the bots
//! invited to it may be told of, leads with `[draft H-nnn “title”]`.

use bus::contract::board as c;

use super::Conn;
use crate::decisions::{invalid, not_found};
use crate::kanban::feed::{card_after_commit, Change, ChangeKind};
use crate::kanban::model::{ColumnCategory, ItemState};

/// The longest message text the card's comment quotes.
const QUOTED_MAX: usize = 500;

/// Where the card's board lives.
pub(super) enum CardBoard {
    Here {
        project_id: String,
        /// The draft's title, when the card is a draft.
        draft: Option<String>,
    },
    /// Mirrored from its home: the comment is the home's to post.
    Mirrored,
}

/// The message as stored and delivered: the card first, so the bot reads it.
pub(super) fn with_card(item_id: &str, body: &str) -> String {
    format!("[card {item_id}] {body}")
}

/// The owner's message about `item_id` as the bot reads it: a draft by its
/// tag, which names its title, any other card by its id.
pub(super) fn leading(item_id: &str, board: &CardBoard, body: &str) -> String {
    match board {
        CardBoard::Here {
            draft: Some(title), ..
        } => format!("{} {body}", crate::kanban::draft_talk::tag(item_id, title)),
        _ => with_card(item_id, body),
    }
}

impl Conn {
    /// The card, if it is on `bot`'s project's board and not closed.
    pub(super) fn owner_card(&self, bot: &bus::Bot, item_id: &str) -> anyhow::Result<CardBoard> {
        let db = &self.app.db;
        let wrong_project = || {
            invalid(format!(
                "{item_id} isn't on the board of {}'s project",
                bot.name
            ))
        };
        if let Some(project_id) = db.board_read(|t| t.item_project(item_id))? {
            if project_id != bot.project_id {
                return Err(wrong_project());
            }
            let item = db
                .board_read(|t| t.item(item_id))?
                .ok_or_else(|| not_found(format!("no card {item_id}")))?;
            // A draft is discussed with the bots invited to it (0.19.0 K4b).
            if item.state != ItemState::Live {
                let invited = db.board_read(|t| t.invited(item_id))?;
                if item.state == ItemState::Discarded {
                    return Err(invalid(format!("{item_id} was discarded")));
                }
                if !invited.contains(&bot.id) {
                    return Err(invalid(format!(
                        "{item_id} is a draft: invite {} to discuss it first",
                        bot.name
                    )));
                }
                return Ok(CardBoard::Here {
                    project_id,
                    draft: Some(item.title),
                });
            }
            let closed = db.board_columns(&project_id)?.iter().any(|col| {
                col.key == item.column_key
                    && matches!(
                        col.category,
                        ColumnCategory::Done | ColumnCategory::Cancelled
                    )
            });
            if closed {
                return Err(invalid(format!("{item_id} is closed")));
            }
            return Ok(CardBoard::Here {
                project_id,
                draft: None,
            });
        }
        let Some(board) = self.app.board_mirror.get(&bot.project_id) else {
            return match self.app.board_mirror.project_of(item_id) {
                Some(_) => Err(wrong_project()),
                None => Err(not_found(format!("no card {item_id}"))),
            };
        };
        let snapshot = board.snapshot;
        let Some(card) = snapshot.cards.iter().find(|card| card.id == item_id) else {
            return Err(match self.app.board_mirror.project_of(item_id) {
                Some(_) => wrong_project(),
                None => not_found(format!("no card {item_id}")),
            });
        };
        let closed = snapshot.columns.iter().any(|col| {
            col.key == card.column_key
                && matches!(
                    col.category(),
                    c::ColumnCategory::Done | c::ColumnCategory::Cancelled
                )
        });
        if closed {
            return Err(invalid(format!("{item_id} is closed")));
        }
        Ok(CardBoard::Mirrored)
    }

    /// "Owner asked <bot>: …" on the card, pushed like the drawer's
    /// comments. `false` when its board lives on another computer.
    pub(super) fn comment_owner_card(
        &self,
        board: &CardBoard,
        bot: &bus::Bot,
        item_id: &str,
        body: &str,
    ) -> anyhow::Result<bool> {
        let CardBoard::Here { project_id, .. } = board else {
            return Ok(false);
        };
        let db = &self.app.db;
        let text = format!(
            "Owner asked {}: {}",
            bot.name,
            crate::attention::cut_text(body, QUOTED_MAX)
        );
        let mut feed = self.app.board.writer();
        db.add_item_comment(item_id, &text, None, &self.actor())?;
        feed.publish(Change {
            project_id,
            kind: ChangeKind::ItemUpserted,
            item_id,
            card: card_after_commit(db, item_id),
            from_column: None,
        });
        Ok(true)
    }
}
