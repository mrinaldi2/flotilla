//! The question tools (0.22.0, `questions_v1`): a bot asks the owner, one
//! record per question, and reads the project's questions. Answering and
//! dismissing are the owner's, in the apps.

use std::sync::Arc;

use bus::contract::board as c;
use serde_json::Value;

use super::board::out;
use super::board_schema::{decode, shared, Audience, BoardTool};
use crate::app::AppState;

pub(super) const QUESTION_TOOLS: &[BoardTool] = &[
    shared(
        "ask_owner",
        "AskOwner",
        Audience::Everyone,
        "Ask the owner, one entry per question, about a card (item_id) and/or a plan node \
         (plan_node_id): each gets its own record the owner answers once, on the card or in \
         Needs you, and you are told each answer. Give a question its options and the one you \
         recommend when there are choices, and `why` in a line. Mark `blocking` a question its \
         node's research or document can't go on without: its research won't close and its \
         gate can't be submitted until it is answered. At most 8 at once.",
    ),
    shared(
        "question_list",
        "QuestionList",
        Audience::Everyone,
        "The project's questions for the owner, open ones first: filter by card, plan node or \
         state. An answered one carries the owner's answer.",
    ),
];

pub(super) fn handles(name: &str) -> bool {
    QUESTION_TOOLS.iter().any(|t| t.name == name)
}

/// A question change's answer: the questions, or why not.
fn questioned(result: c::QuestionResult) -> anyhow::Result<Value> {
    match result.outcome {
        Some(c::question_result::Outcome::Done(done)) => out(done),
        Some(c::question_result::Outcome::Refused(r)) => {
            Err(anyhow::anyhow!("{}: {}", r.code, r.text))
        }
        Some(c::question_result::Outcome::Conflict(now)) => Err(anyhow::anyhow!(
            "conflict: question {} changed; it is at version {} now",
            now.id,
            now.version
        )),
        None => Err(anyhow::anyhow!("no outcome")),
    }
}

pub(super) fn call(
    app: &Arc<AppState>,
    bot: &bus::Bot,
    name: &str,
    args: &Value,
) -> anyhow::Result<Value> {
    let project = bot.project_id.as_str();
    match name {
        "ask_owner" => questioned(crate::questions::ask(
            app,
            bot,
            &decode("AskOwner", args, project)?,
        )?),
        "question_list" => {
            let mut req: c::QuestionList = decode("QuestionList", args, project)?;
            req.project_id = project.to_string();
            out(crate::questions::list(app, &req)?)
        }
        other => Err(anyhow::anyhow!("unknown question tool {other}")),
    }
}
