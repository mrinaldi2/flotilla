//! The plan's rulings and guards (0.21.0, #43): a send-back needs a note
//! and starts the next round, the lead's skip asks the owner, a drop takes
//! the subtree, a stale version is a conflict, and a goal moves by hand.

use crate::common;

use bus::contract::board as c;
use common::plan::*;
use flotillad::plan::model::{ResearchState, Stage};
use flotillad::plan::Outcome;

#[tokio::test]
async fn a_send_back_needs_a_note_and_the_next_submission_is_round_two() {
    let plan = planned().await;
    let goal = plan.goal("Ship the demo");
    let init = plan.skip(&plan.initiative(&goal.id, "Combat readability"));
    let path = "docs/initiatives/combat/prd.md";
    let submitted = plan.submit(&init, path, &["Telegraphing"]);
    let send_back = |note: Option<&str>| {
        plan.owner().rule(&c::PlanRule {
            id: submitted.id.clone(),
            expected_version: submitted.version,
            approve: false,
            note: note.map(str::to_string),
            ..Default::default()
        })
    };
    assert_eq!(refused(send_back(None)), ["plan.note"]);
    assert_eq!(refused(send_back(Some("  "))), ["plan.note"]);
    let back = done(send_back(Some("Say how we measure it")));
    assert_eq!(back.stage, Stage::Prd, "it stays at its gate");
    assert!(back.gate.is_none());
    let again = plan.submit(&back, path, &["Telegraphing"]);
    assert_eq!(again.gate.as_ref().unwrap().round, 2);
    let events = plan.owner().detail(&init.id).unwrap().unwrap().history;
    let sent = events.iter().find(|e| e.kind == "sent_back").unwrap();
    assert_eq!(sent.note.as_deref(), Some("Say how we measure it"));
    // Nothing waits: there is nothing to rule on.
    let (approved, _) = plan.approve(&again, &[]);
    let nothing = plan.owner().rule(&c::PlanRule {
        id: approved.id.clone(),
        expected_version: approved.version,
        approve: true,
        ..Default::default()
    });
    assert_eq!(refused(nothing), ["plan.no_gate"]);
    // A path outside docs/ or with `..` never reaches git.
    for bad in ["src/main.rs", "docs/../secret", "/docs/x.md"] {
        let e = plan
            .lead()
            .submit(&c::PlanSubmit {
                id: approved.id.clone(),
                expected_version: approved.version,
                doc_path: bad.into(),
                ..Default::default()
            })
            .unwrap_err();
        assert_eq!(
            flotillad::decisions::error_code(&e),
            Some("invalid_request"),
            "{bad}"
        );
    }
}

#[tokio::test]
async fn the_leads_skip_asks_the_owner_and_the_owners_skip_is_at_once() {
    let plan = planned().await;
    let goal = plan.goal("Ship the demo");
    let init = plan.initiative(&goal.id, "Combat readability");
    let ask = |node: &flotillad::plan::model::Node, reason: &str| {
        plan.lead().research_skip(&c::PlanResearchSkip {
            id: node.id.clone(),
            expected_version: node.version,
            reason: reason.into(),
        })
    };
    assert_eq!(refused(ask(&init, " ")), ["plan.reason"]);
    let asked = done(ask(&init, "We built this before"));
    assert_eq!(asked.stage, Stage::Research, "still researching");
    let gate = asked.gate.clone().expect("a gate for the owner");
    assert_eq!(gate.stage, Stage::Research);
    assert_eq!(gate.note.as_deref(), Some("We built this before"));
    assert_eq!(refused(ask(&asked, "Again")), ["plan.gate_waiting"]);
    let rows = flotillad::overview::attention_rows(&plan.d.app, &plan.pid).unwrap();
    assert!(rows
        .rows
        .iter()
        .any(|r| r.title == "Skip research for Combat readability?"));
    let (skipped, _) = plan.approve(&asked, &[]);
    assert_eq!(skipped.stage, Stage::Prd);
    let research = skipped.research.unwrap();
    assert_eq!(research.state, ResearchState::Skipped);
    assert_eq!(
        research.skipped_reason.as_deref(),
        Some("We built this before")
    );

    // The owner's own skip needs no ruling.
    let other = plan.initiative(&goal.id, "Netcode");
    let skipped = plan.skip(&other);
    assert_eq!(skipped.stage, Stage::Prd);
    assert!(skipped.gate.is_none());
    let again = plan.owner().research_skip(&c::PlanResearchSkip {
        id: skipped.id.clone(),
        expected_version: skipped.version,
        reason: "twice".into(),
    });
    assert_eq!(refused(again), ["plan.research_state"]);
}

#[tokio::test]
async fn a_drop_takes_the_subtree_and_keeps_its_cards() {
    let plan = planned().await;
    let goal = plan.goal("Ship the demo");
    let init = plan.skip(&plan.initiative(&goal.id, "Combat readability"));
    let init = plan.submit(&init, "docs/initiatives/combat/prd.md", &["A", "B"]);
    let (init, epics) = plan.approve(&init, &[]);
    let spike = plan.spike(&plan.node(&epics[0]), "Look around");
    let by_bot = plan.lead().drop_node(&c::PlanDrop {
        id: init.id.clone(),
        expected_version: init.version,
        reason: None,
    });
    assert_eq!(refused(by_bot), ["plan.owner_only"]);
    let dropped = done(plan.owner().drop_node(&c::PlanDrop {
        id: init.id.clone(),
        expected_version: init.version,
        reason: Some("Out of scope".into()),
    }));
    assert_eq!(dropped.stage, Stage::Dropped);
    for epic in &epics {
        let epic = plan.node(epic);
        assert_eq!(epic.stage, Stage::Dropped);
    }
    assert_eq!(plan.node(&goal.id).stage, Stage::Define, "its parent stays");
    assert!(
        plan.d.app.db.get_item(&spike).unwrap().is_some(),
        "cards stay"
    );
    let event = plan
        .owner()
        .detail(&epics[1])
        .unwrap()
        .unwrap()
        .history
        .pop()
        .unwrap();
    assert_eq!(
        (event.kind.as_str(), event.note.as_deref()),
        ("dropped", Some("Out of scope"))
    );
}

#[tokio::test]
async fn a_stale_version_is_a_conflict_with_the_node_as_it_is() {
    let plan = planned().await;
    let goal = plan.goal("Ship the demo");
    let edited = done(plan.owner().update(&c::PlanUpdate {
        id: goal.id.clone(),
        expected_version: goal.version,
        title: Some("Ship the playtest".into()),
        due: Some("Playtest round 2".into()),
        ..Default::default()
    }));
    assert_eq!(edited.version, goal.version + 1);
    assert_eq!(edited.due.as_deref(), Some("Playtest round 2"));
    let stale = plan
        .owner()
        .update(&c::PlanUpdate {
            id: goal.id.clone(),
            expected_version: goal.version,
            title: Some("Other".into()),
            ..Default::default()
        })
        .unwrap();
    match stale.outcome {
        Outcome::Conflict(now) => assert_eq!(now.title, "Ship the playtest"),
        other => panic!("{other:?}"),
    }

    // A goal moves define → active → met by hand, and nothing else does.
    let stage = |node: &flotillad::plan::model::Node, to: c::PlanStage| {
        plan.owner().update(&c::PlanUpdate {
            id: node.id.clone(),
            expected_version: node.version,
            stage: Some(to as i32),
            ..Default::default()
        })
    };
    assert_eq!(refused(stage(&edited, c::PlanStage::Met)), ["plan.stage"]);
    let active = done(stage(&edited, c::PlanStage::Active));
    assert_eq!(active.stage, Stage::Active);
    let by_lead = plan.lead().update(&c::PlanUpdate {
        id: active.id.clone(),
        expected_version: active.version,
        stage: Some(c::PlanStage::Met as i32),
        ..Default::default()
    });
    assert_eq!(refused(by_lead), ["plan.owner_only"]);
    let init = plan.initiative(&active.id, "Combat readability");
    assert_eq!(refused(stage(&init, c::PlanStage::Prd)), ["plan.stage"]);
    // The lead edits what it leads.
    let led = done(plan.lead().update(&c::PlanUpdate {
        id: init.id.clone(),
        expected_version: init.version,
        summary: Some("Players read every attack.".into()),
        ..Default::default()
    }));
    assert_eq!(led.summary, "Players read every attack.");
    let met = done(stage(&active, c::PlanStage::Met));
    assert_eq!(met.stage, Stage::Met);
}
