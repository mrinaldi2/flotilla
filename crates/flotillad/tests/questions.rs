//! Questions for the owner (0.22.0, `questions_v1`): one record per
//! question on a card or plan node, answered once, written back into the
//! research document, and holding a node's research and gate when blocking.

use crate::common;

use bus::contract::board as c;
use common::plan::*;
use flotillad::actor::Actor;
use flotillad::kanban::attach::Source;
use flotillad::kanban::edit::Editor;
use flotillad::plan::model::{Node, ResearchState, Stage};
use flotillad::questions;

fn lead_bot(plan: &Plan) -> bus::Bot {
    plan.d
        .app
        .db
        .get_bot(&plan.lead)
        .unwrap()
        .expect("the lead")
}

fn ask(text: &str, blocking: bool) -> c::QuestionAsk {
    c::QuestionAsk {
        text: text.into(),
        why: Some("It sets the dataset's size.".into()),
        options: vec![
            c::QuestionOption {
                key: "celebrated".into(),
                label: "The celebrated saints".into(),
                description: None,
            },
            c::QuestionOption {
                key: "full".into(),
                label: "The full Martyrology".into(),
                description: None,
            },
        ],
        recommendation: Some("celebrated".into()),
        blocking,
    }
}

fn asked(result: c::QuestionResult) -> Vec<c::Question> {
    match result.outcome {
        Some(c::question_result::Outcome::Done(done)) => done.questions,
        other => panic!("not done: {other:?}"),
    }
}

fn refused_code(result: c::QuestionResult) -> String {
    match result.outcome {
        Some(c::question_result::Outcome::Refused(r)) => r.code,
        other => panic!("not refused: {other:?}"),
    }
}

/// The owner's answer to `q`.
fn answer(plan: &Plan, q: &c::Question, option: Option<&str>, text: &str) -> c::QuestionResult {
    questions::answer(
        &plan.d.app,
        &Actor::User,
        &c::QuestionAnswer {
            id: q.id.clone(),
            expected_version: q.version,
            option: option.map(str::to_string),
            text: text.into(),
        },
    )
    .unwrap()
}

/// The messages in `bot`'s conversation with the owner.
fn told(plan: &Plan, bot: &str) -> Vec<String> {
    let db = &plan.d.app.db;
    let Some(conv) = db.dm_conversation(bot).unwrap() else {
        return Vec::new();
    };
    db.list_messages(&conv.id, None, 100)
        .unwrap()
        .into_iter()
        .map(|m| m.body)
        .collect()
}

/// Attach `text` to the card as `name`, as the lead would.
fn attach_doc(plan: &Plan, card: &str, name: &str, text: &str) {
    let dir = plan._remote.path().join("attach");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join(name);
    std::fs::write(&file, text).unwrap();
    Editor {
        app: &plan.d.app,
        project_id: &plan.pid,
        actor: Actor::User,
    }
    .attach(
        &c::ItemAttach {
            id: card.into(),
            ..Default::default()
        },
        Source::File {
            path: file,
            roots: vec![dir.canonicalize().unwrap()],
        },
    )
    .unwrap();
}

/// The text of the card's newest attachment still on it.
fn research_text(plan: &Plan, card: &str) -> String {
    let db = &plan.d.app.db;
    let att = db
        .board_read(|t| t.attachments(card))
        .unwrap()
        .into_iter()
        .filter(|a| !a.removed)
        .max_by_key(|a| a.at)
        .expect("an attachment");
    let artifacts = flotillad::kanban::attach::artifacts_of(&plan.d.app, &plan.pid).unwrap();
    std::fs::read_to_string(artifacts.join(att.path)).unwrap()
}

fn initiative_in_research(plan: &Plan) -> Node {
    let goal = plan.goal("A research of all saints");
    plan.initiative(&goal.id, "Sources & licensing")
}

#[tokio::test]
async fn each_question_is_its_own_record_on_the_card() {
    let plan = planned().await;
    let init = initiative_in_research(&plan);
    let spike = plan.spike(&init, "Sources");
    let lead = lead_bot(&plan);
    let mut second = ask("Italy plus the universal calendar only?", false);
    second.options.clear();
    second.recommendation = None;
    let qs = asked(
        questions::ask(
            &plan.d.app,
            &lead,
            &c::AskOwner {
                item_id: Some(spike.clone()),
                plan_node_id: None,
                questions: vec![ask("Only the celebrated saints?", false), second],
            },
        )
        .unwrap(),
    );
    assert_eq!(qs.len(), 2);
    assert!(qs.iter().all(|q| q.state == c::QuestionState::Open as i32));
    assert!(qs
        .iter()
        .all(|q| q.item_id.as_deref() == Some(spike.as_str())));
    assert_eq!(qs[0].asked_by.as_ref().unwrap().bot_id, plan.lead);
    assert_eq!(qs[0].recommendation.as_deref(), Some("celebrated"));

    // The card's comment links them, and the card counts them.
    let comments = plan
        .d
        .app
        .db
        .board_read(|t| t.item_comments(&spike))
        .unwrap();
    let note = comments.last().expect("a comment");
    assert!(note.body.contains("2 questions"), "{}", note.body);
    assert!(note.body.contains(&qs[1].id), "{}", note.body);
    assert_eq!(qs[0].comment_id.as_deref(), Some(note.id.as_str()));
    let on_card = questions::on(&plan.d.app, Some(&spike), None).unwrap();
    assert_eq!(on_card.len(), 2);
    let node = plan.node(&init.id);
    assert_eq!((node.questions_open, node.questions_blocking), (2, 0));

    // The registry carries the card; Needs you has a row each.
    let decision = plan.d.app.db.get_decision(&qs[0].id).unwrap().unwrap();
    let view = flotillad::decisions::decision_view(
        &plan.d.app.db,
        &decision,
        flotillad::decisions::Detail::Summary,
    )
    .unwrap();
    assert_eq!(view.item_id.as_deref(), Some(spike.as_str()));
    let rows = flotillad::overview::attention_rows(&plan.d.app, &plan.pid).unwrap();
    let question_rows = rows
        .rows
        .iter()
        .filter(|r| r.kind == bus::contract::home::AttentionKind::Question as i32)
        .count();
    assert_eq!(question_rows, 2);
}

#[tokio::test]
async fn an_answer_is_given_once_and_written_into_the_research() {
    let plan = planned().await;
    let init = initiative_in_research(&plan);
    let spike = plan.spike(&init, "Sources");
    attach_doc(
        &plan,
        &spike,
        "sources-research.md",
        "# Sources\n\nLayers of authority.\n",
    );
    let lead = lead_bot(&plan);
    let q = asked(
        questions::ask(
            &plan.d.app,
            &lead,
            &c::AskOwner {
                item_id: Some(spike.clone()),
                questions: vec![ask("Only the celebrated saints?", false)],
                ..Default::default()
            },
        )
        .unwrap(),
    )
    .remove(0);

    // A stale version, an unknown option and an empty answer are refused.
    let stale = c::Question {
        version: q.version + 7,
        ..q.clone()
    };
    assert!(matches!(
        answer(&plan, &stale, None, "yes").outcome,
        Some(c::question_result::Outcome::Conflict(_))
    ));
    assert_eq!(
        refused_code(answer(&plan, &q, Some("most"), "")),
        "question.no_option"
    );
    assert_eq!(refused_code(answer(&plan, &q, None, " ")), "question.empty");

    // Picking an option is an answer; its label stands for the text.
    let after = asked(answer(&plan, &q, Some("celebrated"), "")).remove(0);
    assert_eq!(after.state, c::QuestionState::Answered as i32);
    let given = after.answer.as_ref().expect("an answer");
    assert_eq!(given.option.as_deref(), Some("celebrated"));
    assert_eq!(given.text, "The celebrated saints");
    assert_eq!(
        refused_code(answer(&plan, &after, Some("full"), "")),
        "question.answered"
    );

    // The bot that asked is told, and the research document has it.
    let notes = told(&plan, &plan.lead);
    assert!(
        notes.iter().any(|n| n.contains("The celebrated saints")),
        "{notes:?}"
    );
    let text = research_text(&plan, &spike);
    assert!(text.starts_with("# Sources"), "{text}");
    assert!(text.contains("## Answers from the owner"), "{text}");
    assert!(text.contains("**Only the celebrated saints?**"), "{text}");
    assert!(after.written_to.is_some());
    // The first revision stays in the card's history, detached.
    let all = plan.d.app.db.board_read(|t| t.attachments(&spike)).unwrap();
    assert_eq!(all.iter().filter(|a| a.removed).count(), 1);
    assert_eq!(all.iter().filter(|a| !a.removed).count(), 1);
}

#[tokio::test]
async fn a_blocking_question_holds_research_and_the_gate() {
    let plan = planned().await;
    let init = initiative_in_research(&plan);
    plan.research_kickoff_done(&init.id);
    let spike = plan.spike(&init, "Sources");
    let lead = lead_bot(&plan);
    // A blocking question needs a node.
    let loose = questions::ask(
        &plan.d.app,
        &lead,
        &c::AskOwner {
            questions: vec![ask("Anything?", true)],
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(refused_code(loose), "question.not_planned");
    let q = asked(
        questions::ask(
            &plan.d.app,
            &lead,
            &c::AskOwner {
                item_id: Some(spike.clone()),
                questions: vec![ask("Only the celebrated saints?", true)],
                ..Default::default()
            },
        )
        .unwrap(),
    )
    .remove(0);
    plan.finish(&spike);
    plan.document(&spike, "research/sources.md");

    // The spikes are done, but the question holds research.
    plan.owner().tree(&plan.pid).unwrap();
    let held = plan.node(&init.id);
    assert_eq!(held.stage, Stage::Research);
    assert_eq!(held.questions_blocking, 1);
    let early = plan.lead().submit(&c::PlanSubmit {
        id: init.id.clone(),
        expected_version: held.version,
        doc_path: "docs/initiatives/combat/prd.md".into(),
        ..Default::default()
    });
    assert_eq!(refused(early), ["plan.question_open"]);

    // Answered, research closes and the PRD is next.
    asked(answer(&plan, &q, None, "Start with the celebrated ones."));
    let node = plan.node(&init.id);
    assert_eq!(node.stage, Stage::Prd);
    assert_eq!(node.research.as_ref().unwrap().state, ResearchState::Closed);
    assert_eq!(node.questions_blocking, 0);
}

#[tokio::test]
async fn the_owner_dismisses_a_question_with_a_reason() {
    let plan = planned().await;
    let init = initiative_in_research(&plan);
    let lead = lead_bot(&plan);
    let q = asked(
        questions::ask(
            &plan.d.app,
            &lead,
            &c::AskOwner {
                plan_node_id: Some(init.id.clone()),
                questions: vec![ask("May we contact the LEV?", true)],
                ..Default::default()
            },
        )
        .unwrap(),
    )
    .remove(0);
    let dismiss = |reason: &str, version: u64| {
        questions::dismiss(
            &plan.d.app,
            &Actor::User,
            &c::QuestionDismiss {
                id: q.id.clone(),
                expected_version: version,
                reason: reason.into(),
            },
        )
        .unwrap()
    };
    assert_eq!(refused_code(dismiss(" ", q.version)), "question.empty");
    // Only the owner dismisses.
    let by_lead = questions::dismiss(
        &plan.d.app,
        &Actor::Bot {
            id: &plan.lead,
            project_id: &plan.pid,
        },
        &c::QuestionDismiss {
            id: q.id.clone(),
            expected_version: q.version,
            reason: "no".into(),
        },
    );
    assert!(by_lead.is_err());
    let gone = asked(dismiss("Settled in the PRD already.", q.version)).remove(0);
    assert_eq!(gone.state, c::QuestionState::Dismissed as i32);
    assert_eq!(
        gone.dismissed_reason.as_deref(),
        Some("Settled in the PRD already.")
    );
    let notes = told(&plan, &plan.lead);
    assert!(
        notes.iter().any(|n| n.contains("dismissed your question")),
        "{notes:?}"
    );
    assert_eq!(plan.node(&init.id).questions_blocking, 0);

    // The list shows it under answered and dismissed, not open.
    let open = questions::list(
        &plan.d.app,
        &c::QuestionList {
            project_id: plan.pid.clone(),
            state: Some(c::QuestionState::Open as i32),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(open.questions.is_empty());
    let dismissed = questions::list(
        &plan.d.app,
        &c::QuestionList {
            project_id: plan.pid.clone(),
            state: Some(c::QuestionState::Dismissed as i32),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(dismissed.questions.len(), 1);
}
