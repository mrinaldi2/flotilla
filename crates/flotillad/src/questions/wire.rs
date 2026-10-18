//! A question as the board's contract carries it (`questions.proto`): its
//! decision and its `question` row, named for the owner.

use bus::contract::board as c;
use bus::{Decision, DecisionState};

use crate::app::AppState;
use crate::db::{Db, QuestionRow};
use crate::kanban::contract::stamp;

/// The question's state, from its decision's.
pub fn state(decision: &Decision) -> c::QuestionState {
    match decision.state {
        DecisionState::Open | DecisionState::Held => c::QuestionState::Open,
        DecisionState::Answered | DecisionState::Settled => c::QuestionState::Answered,
        DecisionState::Withdrawn => c::QuestionState::Dismissed,
    }
}

/// The wire question of a decision and its row.
pub fn question(app: &AppState, decision: &Decision, row: &QuestionRow) -> c::Question {
    let asker = app.db.get_bot(&decision.raised_by_bot_id).ok().flatten();
    let state = state(decision);
    c::Question {
        id: decision.id.clone(),
        project_id: decision.project_id.clone(),
        text: decision.title.clone(),
        why: decision.body.clone(),
        options: decision
            .options
            .iter()
            .map(|o| c::QuestionOption {
                key: o.key.clone(),
                label: o.label.clone(),
                description: o.description.clone(),
            })
            .collect(),
        recommendation: decision.recommendation.clone(),
        blocking: row.blocking,
        state: state as i32,
        asked_by: Some(c::QuestionAsker {
            bot_id: decision.raised_by_bot_id.clone(),
            name: asker
                .as_ref()
                .map(Db::display_name)
                .unwrap_or_else(|| "a deleted bot".to_string()),
            avatar: asker.map(|b| b.avatar).unwrap_or_default(),
        }),
        asked_at: stamp(decision.created_at),
        item_id: row.item_id.clone(),
        plan_node_id: row.plan_node_id.clone(),
        answer: match state {
            c::QuestionState::Answered => decision.ruling.as_ref().map(|r| c::OwnerAnswer {
                option: r.option.clone(),
                text: r.text.clone(),
                answered_by: r.answered_by.clone(),
                answered_at: stamp(r.answered_at),
            }),
            _ => None,
        },
        dismissed_reason: row.dismissed_reason.clone(),
        written_to: row.written_to.clone(),
        comment_id: row.comment_id.clone(),
        version: row.version,
    }
}

/// The question of `id` as it is now, if it is one.
pub fn load(app: &AppState, id: &str) -> anyhow::Result<Option<c::Question>> {
    let (Some(decision), Some(row)) = (app.db.get_decision(id)?, app.db.question_row(id)?) else {
        return Ok(None);
    };
    Ok(Some(question(app, &decision, &row)))
}

/// Questions by id, in that order, skipping any gone since.
pub fn load_all(app: &AppState, ids: &[String]) -> anyhow::Result<Vec<c::Question>> {
    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        if let Some(q) = load(app, id)? {
            out.push(q);
        }
    }
    Ok(out)
}

/// The question state's short and wire names, for bots: they filter by
/// `open`, `answered` or `dismissed` and read the same.
pub fn spellings(enum_name: &str) -> Option<Vec<(&'static str, &'static str)>> {
    (enum_name == "QuestionState").then(all_spellings)
}

pub fn all_spellings() -> Vec<(&'static str, &'static str)> {
    [
        c::QuestionState::Open,
        c::QuestionState::Answered,
        c::QuestionState::Dismissed,
    ]
    .iter()
    .map(|s| {
        let wire = s.as_str_name();
        (
            match s {
                c::QuestionState::Open => "open",
                c::QuestionState::Answered => "answered",
                _ => "dismissed",
            },
            wire,
        )
    })
    .collect()
}
