//! Questions for the owner (0.22.0, `questions_v1`, questions.proto). A bot
//! asks with `ask_owner`, one decision of kind `question` per question, on a
//! card, a plan node or both. The owner answers each once where it shows
//! (the card, the plan node, the project's list, Needs you) or dismisses it
//! with a reason. An answer is published at once: the bot that asked is
//! told, and so is the lead for plan work, and a research spike's answers
//! are written into its research document. A blocking question holds its
//! node's research and gate while it is open (`plan::research`,
//! `plan::gates`).

use std::sync::Arc;

use bus::contract::board as c;
use bus::{DecisionKind, DecisionOption, DecisionState, Priority};

use crate::actor::Actor;
use crate::app::AppState;
use crate::db::{NewQuestion, QuestionFilter};
use crate::decisions::RaiseRequest;

mod checks;
mod notes;
pub mod wire;
mod writeback;

/// At most this many questions in one `ask_owner`.
pub const MAX_PER_ASK: usize = 8;

pub(super) fn refused(code: &str, text: impl Into<String>) -> c::QuestionResult {
    c::QuestionResult {
        outcome: Some(c::question_result::Outcome::Refused(c::QuestionRefused {
            code: code.to_string(),
            text: text.into(),
        })),
    }
}

fn done(questions: Vec<c::Question>) -> c::QuestionResult {
    c::QuestionResult {
        outcome: Some(c::question_result::Outcome::Done(c::QuestionsDone {
            questions,
        })),
    }
}

fn conflict(now: c::Question) -> c::QuestionResult {
    c::QuestionResult {
        outcome: Some(c::question_result::Outcome::Conflict(now)),
    }
}

/// A bot asks the owner: one record per question.
pub fn ask(
    app: &Arc<AppState>,
    bot: &bus::Bot,
    req: &c::AskOwner,
) -> anyhow::Result<c::QuestionResult> {
    let item_id = req
        .item_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let node_id = req
        .plan_node_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    if let Some(no) = checks::anchor_problem(app, &bot.project_id, item_id, node_id)? {
        return Ok(no);
    }
    if req.questions.is_empty() {
        return Ok(refused("question.empty", "ask at least one question"));
    }
    if req.questions.len() > MAX_PER_ASK {
        return Ok(refused(
            "question.too_many",
            format!("ask at most {MAX_PER_ASK} questions at once"),
        ));
    }
    let open = app.db.open_asked_by(&bot.id)?;
    if open + req.questions.len() as i64 > bus::MAX_OPEN_DECISIONS_PER_BOT {
        return Ok(refused(
            "question.too_many",
            format!(
                "you have {open} questions and decisions waiting on the owner; at most {} may \
                 wait at once",
                bus::MAX_OPEN_DECISIONS_PER_BOT
            ),
        ));
    }
    for (i, q) in req.questions.iter().enumerate() {
        if let Some(no) = checks::ask_problem(
            q,
            i,
            node_id.is_some() || checks::item_on_node(app, item_id)?,
        ) {
            return Ok(no);
        }
    }
    let actor = Actor::Bot {
        id: &bot.id,
        project_id: &bot.project_id,
    };
    let mut asked = Vec::with_capacity(req.questions.len());
    for q in &req.questions {
        let options: Vec<DecisionOption> = q
            .options
            .iter()
            .map(|o| DecisionOption {
                key: o.key.trim().to_string(),
                label: o.label.trim().to_string(),
                description: o.description.clone().filter(|d| !d.trim().is_empty()),
                grants: Vec::new(),
                grants_sha: None,
            })
            .collect();
        let raised = crate::decisions::service::raise(
            app,
            bot,
            &RaiseRequest {
                kind: DecisionKind::Question,
                title: &q.text,
                body: q.why.as_deref().unwrap_or_default(),
                options: &options,
                recommendation: q.recommendation.as_deref(),
                tags: &[],
                priority: Priority::Normal,
                deadline_at: None,
                on_behalf_of: None,
                source_task_id: None,
                supersedes: None,
            },
        );
        let raised = match raised {
            Ok(raised) => raised,
            Err(e) if asked.is_empty() => return Ok(refused("question.refused", e.to_string())),
            Err(e) => return Err(e),
        };
        let id = raised.decision.id;
        app.db.insert_question(&NewQuestion {
            decision_id: &id,
            project_id: &bot.project_id,
            item_id,
            plan_node_id: node_id,
            blocking: q.blocking,
        })?;
        if let Some(item) = item_id {
            app.db.link_decision_item(&id, item, &actor)?;
        }
        asked.push(id);
    }
    if let Some(item) = item_id {
        notes::announce(app, item, &asked, &actor)?;
    }
    let questions = wire::load_all(app, &asked)?;
    publish(app, &questions, &format!("bot:{}", bot.id));
    Ok(done(questions))
}

/// The project's questions, open ones first.
pub fn list(app: &AppState, req: &c::QuestionList) -> anyhow::Result<c::QuestionPage> {
    let open = match c::QuestionState::try_from(req.state.unwrap_or(0)) {
        Ok(c::QuestionState::Open) => Some(true),
        Ok(c::QuestionState::Answered | c::QuestionState::Dismissed) => Some(false),
        _ => None,
    };
    let (ids, next) = app.db.question_ids(&QuestionFilter {
        project_id: Some(&req.project_id),
        item_id: req.item_id.as_deref(),
        plan_node_id: req.plan_node_id.as_deref(),
        open,
        before: req.before.as_deref(),
        limit: req.limit,
    })?;
    let mut questions = wire::load_all(app, &ids)?;
    // The open filter matches both answered and dismissed; narrow it.
    if let Some(Ok(state @ (c::QuestionState::Answered | c::QuestionState::Dismissed))) =
        req.state.map(c::QuestionState::try_from)
    {
        questions.retain(|q| q.state == state as i32);
    }
    Ok(c::QuestionPage { questions, next })
}

/// A card's questions, or a node's and its cards', open ones first.
pub fn on(
    app: &AppState,
    item_id: Option<&str>,
    node_id: Option<&str>,
) -> anyhow::Result<Vec<c::Question>> {
    let (ids, _) = app.db.question_ids(&QuestionFilter {
        project_id: None,
        item_id,
        plan_node_id: node_id,
        open: None,
        before: None,
        limit: 200,
    })?;
    wire::load_all(app, &ids)
}

/// The owner answers one question; it is published at once.
pub fn answer(
    app: &Arc<AppState>,
    actor: &Actor<'_>,
    req: &c::QuestionAnswer,
) -> anyhow::Result<c::QuestionResult> {
    crate::decisions::owner_only(actor, "answer a question")?;
    let Some(now) = wire::load(app, &req.id)? else {
        return Err(crate::decisions::not_found(format!(
            "no question {}",
            req.id
        )));
    };
    if now.version != req.expected_version {
        return Ok(conflict(now));
    }
    if now.state != c::QuestionState::Open as i32 {
        return Ok(refused(
            "question.answered",
            "this question was already answered or dismissed",
        ));
    }
    let option = req
        .option
        .as_deref()
        .map(str::trim)
        .filter(|o| !o.is_empty());
    let label = match option {
        Some(key) => match now.options.iter().find(|o| o.key == key) {
            Some(o) => Some(o.label.clone()),
            None => return Ok(refused("question.no_option", format!("no option {key}"))),
        },
        None => None,
    };
    let text = match (req.text.trim(), label) {
        ("", Some(label)) => label,
        ("", None) => {
            return Ok(refused(
                "question.empty",
                "pick an option or write an answer",
            ))
        }
        (text, _) => text.to_string(),
    };
    if !app.db.bump_question(&req.id, req.expected_version)? {
        return Ok(conflict(wire::load(app, &req.id)?.unwrap_or(now)));
    }
    crate::decisions::answer(app, actor, &req.id, option, &text, None)?;
    let notify = notes::who_hears(app, &req.id)?;
    crate::decisions::publish(app, &req.id, Some(&notify))?;
    if let Err(e) = writeback::write(app, &req.id) {
        tracing::warn!(question = %req.id, error = %e, "answer not written to the research document");
    }
    let after = wire::load_all(app, std::slice::from_ref(&req.id))?;
    publish(app, &after, &actor.as_stored());
    settle(app, &after);
    Ok(done(after))
}

/// The owner dismisses one question with a reason; the bot that asked is
/// told.
pub fn dismiss(
    app: &Arc<AppState>,
    actor: &Actor<'_>,
    req: &c::QuestionDismiss,
) -> anyhow::Result<c::QuestionResult> {
    crate::decisions::owner_only(actor, "dismiss a question")?;
    let Some(now) = wire::load(app, &req.id)? else {
        return Err(crate::decisions::not_found(format!(
            "no question {}",
            req.id
        )));
    };
    if now.version != req.expected_version {
        return Ok(conflict(now));
    }
    let reason = req.reason.trim();
    if reason.is_empty() {
        return Ok(refused("question.empty", "say why you dismiss it"));
    }
    let decision = app.db.get_decision(&req.id)?;
    if decision.as_ref().map(|d| d.state) != Some(DecisionState::Open)
        && decision.as_ref().map(|d| d.state) != Some(DecisionState::Held)
    {
        return Ok(refused(
            "question.answered",
            "this question was already answered or dismissed",
        ));
    }
    if !app
        .db
        .dismiss_question(&req.id, req.expected_version, reason, &actor.as_stored())?
    {
        return Ok(conflict(wire::load(app, &req.id)?.unwrap_or(now)));
    }
    notes::tell_dismissed(app, &req.id, reason)?;
    // The decision registry hears of it as of any withdrawal.
    crate::decisions::pushed(app, &req.id)?;
    let after = wire::load_all(app, std::slice::from_ref(&req.id))?;
    publish(app, &after, &actor.as_stored());
    settle(app, &after);
    Ok(done(after))
}

/// A question no longer open may let its node's research close.
fn settle(app: &Arc<AppState>, questions: &[c::Question]) {
    let Some(q) = questions.first() else {
        return;
    };
    let planner = crate::plan::Planner {
        app,
        actor: Actor::Daemon,
    };
    if let Err(e) = planner.settle(&q.project_id) {
        tracing::warn!(project = %q.project_id, error = %e, "plan not settled after a question");
    }
}

/// Push each question, and its node so the counts follow.
fn publish(app: &AppState, questions: &[c::Question], actor: &str) {
    let mut feed = app.board.writer();
    let mut nodes: Vec<String> = Vec::new();
    for q in questions {
        feed.publish_question(q.clone(), Some(actor.to_string()));
        let node = q.plan_node_id.clone().or_else(|| {
            q.item_id
                .as_deref()
                .and_then(|i| app.db.get_item(i).ok().flatten())
                .and_then(|i| i.plan_node_id)
        });
        nodes.extend(node.filter(|n| !nodes.contains(n)));
    }
    for id in nodes {
        if let Ok(Some(node)) = app.db.board_read(|t| t.plan_node(&id)) {
            feed.publish_plan(node, Some(actor.to_string()));
        }
    }
}
