//! The owner's item edits and moves over WebSocket (0.19.0 K4b): the same
//! `kanban::edit` service the bots' MCP tools use, run as the owner. The
//! item's assignee hears about an edit or a move the owner made.

use bus::contract::board::{self as c, board_request::Request, board_response::Response};

use super::{not_found, refuse, Refusal};
use crate::kanban::edit::{Edited, Editor};
use crate::kanban::feed::{card_after_commit, Change, ChangeKind};
use crate::kanban::moves::{self, MoveRequest, Moved};
use crate::kanban::notice_text::Notice;
use crate::kanban::notices::{tell_assignee, tell_reassigned};
use crate::messaging;
use crate::ws::Conn;

impl Conn {
    /// The project an owner's request is about: the item's, or the new
    /// item's.
    fn edit_project(&self, req: &Request) -> Result<String, Refusal> {
        let id = match req {
            Request::ItemCreate(r) => return Ok(r.project_id.clone()),
            Request::ItemUpdate(r) => &r.id,
            Request::ItemLink(r) => &r.id,
            Request::ItemUnlink(r) => &r.id,
            Request::ItemBlock(r) => &r.id,
            Request::ItemUnblock(r) => &r.id,
            Request::ItemAssign(r) => &r.id,
            Request::ItemRank(r) => &r.id,
            Request::ItemCheckAc(r) => &r.id,
            Request::ItemFlagAc(r) => &r.id,
            _ => return Err(refuse("invalid_request", "not an item edit")),
        };
        self.project_of_item(id)
    }

    pub(super) fn project_of_item(&self, id: &str) -> Result<String, Refusal> {
        self.app
            .db
            .board_read(|t| t.item_project(id))?
            .ok_or_else(|| not_found(id))
    }

    pub(super) fn editor<'a>(&'a self, project_id: &'a str) -> Editor<'a> {
        Editor {
            app: &self.app,
            project_id,
            actor: self.actor(),
        }
    }

    /// One of the owner's item edits, answered like a bot's: the item as it
    /// is now, or the guards it didn't meet, or a conflict.
    pub(super) fn owner_edit(&self, req: Request) -> Result<Response, Refusal> {
        let project_id = self.edit_project(&req)?;
        let ed = self.editor(&project_id);
        // Who held it before an assignment, so both ends are told.
        let before = match &req {
            Request::ItemAssign(r) => Some(
                self.app
                    .db
                    .board_read(|t| t.item(&r.id))?
                    .and_then(|i| i.assignee),
            ),
            _ => None,
        };
        let (edited, what) = match &req {
            Request::ItemCreate(r) => (ed.create(r), ""),
            Request::ItemUpdate(r) => (ed.update(r), "edited it"),
            Request::ItemBlock(r) => (ed.block(r), "blocked it"),
            Request::ItemUnblock(r) => (ed.unblock(r), "unblocked it"),
            Request::ItemAssign(r) => (ed.assign(r), ""),
            Request::ItemRank(r) => (ed.rank(r), "ranked it"),
            Request::ItemCheckAc(r) => (
                ed.check_ac(r, &messaging::user_sender()),
                "checked an acceptance criterion",
            ),
            Request::ItemFlagAc(r) => (ed.flag_ac(r), "flagged an acceptance criterion"),
            Request::ItemLink(r) => {
                ed.link(r).map_err(refusal)?;
                return self.edited_now(&r.id, "linked something to it");
            }
            Request::ItemUnlink(r) => {
                ed.unlink(r).map_err(refusal)?;
                return self.edited_now(&r.id, "removed a link");
            }
            _ => return Err(refuse("invalid_request", "not an item edit")),
        };
        let edited = edited.map_err(refusal)?;
        if let Edited::Done(item) = &edited {
            match &before {
                Some(before) => tell_reassigned(&self.app, before.as_deref(), item, &self.actor()),
                None if !what.is_empty() => {
                    tell_assignee(&self.app, item, &self.actor(), Notice::Changed(what));
                }
                None => {}
            }
        }
        Ok(Response::Edited(super::drafts::edit_result(edited)))
    }

    /// An unversioned edit's answer: the item as it is now.
    pub(super) fn edited_now(&self, id: &str, what: &str) -> Result<Response, Refusal> {
        let item = self
            .app
            .db
            .board_read(|t| t.item(id))?
            .ok_or_else(|| not_found(id))?;
        tell_assignee(&self.app, &item, &self.actor(), Notice::Changed(what));
        Ok(Response::Edited(c::EditResult {
            outcome: Some(c::edit_result::Outcome::Done(item.into())),
            told: Vec::new(),
        }))
    }

    /// Run the move through the guards in one transaction, holding the feed
    /// so its push is numbered in commit order.
    pub(super) fn item_move(&self, r: &c::ItemMove) -> Result<c::MoveResult, Refusal> {
        let db = &self.app.db;
        let mut feed = self.app.board.writer();
        let (project_id, from) = db
            .board_read(|t| Ok(t.item_project(&r.id)?.zip(t.item(&r.id)?)))?
            .map(|(project, item)| (project, item.column_key))
            .ok_or_else(|| not_found(&r.id))?;
        let request = MoveRequest {
            id: &r.id,
            to: &r.to,
            expected_version: r.expected_version,
            reason: r.reason.as_deref(),
            override_reason: r.override_reason.as_deref(),
        };
        let moved = moves::item_move(db, &request, &self.actor())?;
        if let Moved::Refused(unmet) = &moved {
            crate::kanban::activity::refused(&self.app, &r.id, &r.to, &self.actor(), unmet);
        }
        if let Moved::Done(item) = &moved {
            let card = card_after_commit(db, &item.id);
            feed.publish(Change {
                project_id: &project_id,
                kind: ChangeKind::ItemMoved,
                item_id: &item.id,
                card,
                from_column: Some(from),
            });
            drop(feed);
            tell_assignee(&self.app, item, &self.actor(), Notice::Moved);
        } else if let Moved::Refused(_) = &moved {
            // The refusal shows on the card ("can't move: …").
            feed.publish(Change {
                project_id: &project_id,
                kind: ChangeKind::ItemUpserted,
                item_id: &r.id,
                card: card_after_commit(db, &r.id),
                from_column: None,
            });
        }
        Ok(super::drafts::move_result(moved))
    }
}

/// A refusal with the service error's code (`invalid_request`, `not_found`,
/// `conflict`, …); anything else is a bad request.
pub(super) fn refusal(e: anyhow::Error) -> Refusal {
    let code = crate::decisions::error_code(&e).unwrap_or("invalid_request");
    refuse(code, e.to_string())
}
