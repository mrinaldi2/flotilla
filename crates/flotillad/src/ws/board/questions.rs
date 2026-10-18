//! Questions for the owner over WebSocket (0.22.0, `questions_v1`): the
//! owner lists, answers and dismisses them. Bots ask over MCP
//! (`ask_owner`), never here.

use bus::contract::board::{self as c, board_request::Request, board_response::Response};

use super::item_edits::refusal;
use super::{refuse, Refusal};
use crate::kanban::feed::BoardChange;
use crate::questions;
use crate::ws::Conn;

impl Conn {
    pub(super) fn question_request(&self, request: Request) -> Result<Response, Refusal> {
        match request {
            Request::QuestionList(r) => Ok(Response::Questions(
                questions::list(&self.app, &r).map_err(refusal)?,
            )),
            Request::QuestionAnswer(r) => Ok(Response::Questioned(
                questions::answer(&self.app, &self.actor(), &r).map_err(refusal)?,
            )),
            Request::QuestionDismiss(r) => Ok(Response::Questioned(
                questions::dismiss(&self.app, &self.actor(), &r).map_err(refusal)?,
            )),
            Request::AskOwner(_) => Err(refuse(
                "invalid_request",
                "ask_owner is a bot's; the owner answers questions",
            )),
            _ => Err(refuse("invalid_request", "not a question request")),
        }
    }
}

/// A question change's push, if the change is one.
pub(super) fn push(change: &BoardChange) -> Option<c::BoardPush> {
    let question = change.question.as_deref()?;
    Some(c::BoardPush {
        push: Some(c::board_push::Push::QuestionChange(c::QuestionChange {
            project_id: change.project_id.clone(),
            question: Some(question.clone()),
            actor: change.actor.clone(),
        })),
    })
}
