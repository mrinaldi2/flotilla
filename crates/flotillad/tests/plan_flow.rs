//! The plan's path from a goal to task cards (0.21.0, #43): planning needs
//! the repository, an initiative gets its board and researches, research
//! closes on its spikes, and each approved gate makes the next level.

use crate::common;

use bus::contract::board as c;
use common::plan::*;
use flotillad::kanban::model::ItemType;
use flotillad::plan::model::{DocKind, Kind, ResearchState, Stage};

#[tokio::test]
async fn planning_needs_the_projects_repository() {
    let plan = unplanned().await;
    plan.d.app.db.set_project_repo(&plan.pid, None).unwrap();
    let why = plan
        .owner()
        .enable(&plan.pid, true)
        .unwrap()
        .expect("refused");
    assert_eq!(why.code, "plan.no_repo");
    let tree = plan.owner().tree(&plan.pid).unwrap();
    assert!(!tree.enabled && !tree.has_repo);

    // Every write but turning it on is refused while it is off.
    let off = plan.owner().create(&c::PlanCreate {
        project_id: plan.pid.clone(),
        kind: c::PlanKind::Goal as i32,
        title: "Ship".into(),
        ..Default::default()
    });
    assert_eq!(refused(off), ["plan.disabled"]);

    let repo = bus::ProjectRepo {
        url: plan.origin.display().to_string(),
        branch: "main".into(),
    };
    plan.d
        .app
        .db
        .set_project_repo(&plan.pid, Some(&repo))
        .unwrap();
    assert!(plan.owner().enable(&plan.pid, true).unwrap().is_none());
    let goal = plan.goal("Ship the demo");
    assert_eq!((goal.id.as_str(), goal.stage), ("PL-1", Stage::Define));
    let tree = plan.owner().tree(&plan.pid).unwrap();
    assert!(tree.enabled && tree.has_repo);
    assert_eq!(tree.nodes.len(), 1);
    let guidelines: Vec<&str> = tree.guidelines.iter().map(|g| g.path.as_str()).collect();
    assert_eq!(guidelines, ["docs/guidelines/style.md"]);
    assert_eq!(tree.guidelines[0].kind, DocKind::Guideline);

    // Off keeps the nodes and hides them.
    assert!(plan.owner().enable(&plan.pid, false).unwrap().is_none());
    assert!(plan.owner().tree(&plan.pid).unwrap().nodes.is_empty());
    assert!(plan.owner().enable(&plan.pid, true).unwrap().is_none());
    assert_eq!(plan.owner().tree(&plan.pid).unwrap().nodes.len(), 1);
    // A bot only proposes; starting a node is the owner's.
    let by_bot = plan.lead().create(&c::PlanCreate {
        project_id: plan.pid.clone(),
        kind: c::PlanKind::Goal as i32,
        title: "Mine".into(),
        ..Default::default()
    });
    assert_eq!(refused(by_bot), ["plan.owner_only"]);
}

#[tokio::test]
async fn an_initiative_gets_its_board_and_research_closes_on_its_spikes() {
    let plan = planned().await;
    let goal = plan.goal("Ship the demo");
    let init = plan.initiative(&goal.id, "Combat readability");
    assert_eq!(init.kind, Kind::Initiative);
    assert_eq!(init.stage, Stage::Research);
    assert_eq!(init.research.as_ref().unwrap().state, ResearchState::Open);
    let board_id = init.board_id.clone().expect("its board");
    let board = plan.d.app.db.board(&board_id).unwrap().expect("a board");
    assert_eq!(
        (board.name.as_str(), board.key.as_str()),
        ("Combat readability", "CR")
    );
    // An epic or a feature comes from approvals only.
    let epic = plan.owner().create(&c::PlanCreate {
        project_id: plan.pid.clone(),
        kind: c::PlanKind::Epic as i32,
        title: "Epic".into(),
        parent_id: Some(init.id.clone()),
        ..Default::default()
    });
    assert_eq!(refused(epic), ["plan.kind"]);

    // Research open refuses the PRD, naming what it waits on.
    let spike = plan.spike(&init, "Look at other games");
    let early = plan.lead().submit(&c::PlanSubmit {
        id: init.id.clone(),
        expected_version: init.version,
        doc_path: "docs/initiatives/combat/prd.md".into(),
        ..Default::default()
    });
    let refusal = match early.unwrap().outcome {
        flotillad::plan::Outcome::Refused(unmet) => unmet,
        other => panic!("{other:?}"),
    };
    assert_eq!(refusal[0].code, "plan.research_open");
    assert!(refusal[0].text.contains(&spike), "{}", refusal[0].text);

    // Done with a document, the spikes (the lead's research kickoff
    // among them) close research and the PRD is next.
    plan.finish(&spike);
    plan.document(&spike, "research/references.md");
    plan.research_kickoff_done(&init.id);
    let tree = plan.owner().tree(&plan.pid).unwrap();
    let read = tree.nodes.iter().find(|n| n.id == init.id).unwrap();
    assert_eq!(read.stage, Stage::Prd);
    let research = read.research.as_ref().unwrap();
    assert_eq!(research.state, ResearchState::Closed);
    assert!(research.closed_at.is_some());
    assert!(research.spikes.contains(&spike), "{:?}", research.spikes);
    assert_eq!(research.spikes.len(), 2, "with the lead's research kickoff");
    let doc = read
        .docs
        .iter()
        .find(|d| d.kind == DocKind::Research && d.path == "research/references.md")
        .unwrap();
    assert_eq!(doc.item_id.as_deref(), Some(spike.as_str()));
    assert_eq!((read.cards_total, read.cards_done), (1, 1));
    let detail = plan.owner().detail(&init.id).unwrap().unwrap();
    let kinds: Vec<&str> = detail.history.iter().map(|e| e.kind.as_str()).collect();
    assert_eq!(kinds, ["created", "research_closed"]);
    // The test's spike, the research kickoff and the PRD's kickoff.
    assert_eq!(detail.cards.len(), 3);
}

#[tokio::test]
async fn approved_gates_make_epics_features_and_task_cards() {
    let plan = planned().await;
    let goal = plan.goal("Ship the demo");
    let init = plan.skip(&plan.initiative(&goal.id, "Combat readability"));
    assert_eq!(init.stage, Stage::Prd);
    assert_eq!(
        init.research.as_ref().unwrap().state,
        ResearchState::Skipped
    );

    // The PRD proposes three epics; the owner leaves the second out.
    let path = "docs/initiatives/combat/prd.md";
    let submitted = plan.submit(&init, path, &["Telegraphing", "Hit stop", "Audio cues"]);
    let gate = submitted.gate.clone().expect("a gate");
    assert_eq!((gate.stage, gate.round), (Stage::Prd, 1));
    let head = common::repo::git(&plan.origin, &["rev-parse", "main"]);
    assert_eq!(
        gate.sha.as_deref(),
        Some(head.trim()),
        "the default branch's head"
    );
    let (init, created) = plan.approve(&submitted, &[1]);
    assert_eq!(init.stage, Stage::Running);
    assert!(init.gate.is_none());
    let prd = init.docs.iter().find(|d| d.kind == DocKind::Prd).unwrap();
    assert_eq!(prd.approved_sha.as_deref(), Some(head.trim()));
    assert_eq!(created.len(), 2);
    let epics: Vec<_> = created.iter().map(|id| plan.node(id)).collect();
    let titles: Vec<&str> = epics.iter().map(|e| e.title.as_str()).collect();
    assert_eq!(titles, ["Telegraphing", "Audio cues"]);
    for epic in &epics {
        assert_eq!((epic.kind, epic.stage), (Kind::Epic, Stage::Research));
        assert_eq!(epic.parent_id.as_deref(), Some(init.id.as_str()));
        assert_eq!(epic.board_id, init.board_id, "the initiative's board");
    }

    // An epic: design, then the HLD with its features.
    let epic = plan.skip(&epics[0]);
    assert_eq!(epic.stage, Stage::Design);
    let epic = plan.submit(&epic, "docs/initiatives/combat/epics/t/design.md", &[]);
    let (epic, _) = plan.approve(&epic, &[]);
    assert_eq!(epic.stage, Stage::Hld);
    let epic = plan.submit(
        &epic,
        "docs/initiatives/combat/epics/t/hld.md",
        &["Wind-up cue"],
    );
    let (epic, created) = plan.approve(&epic, &[]);
    assert_eq!(epic.stage, Stage::Running);
    let feature = plan.node(&created[0]);
    assert_eq!((feature.kind, feature.stage), (Kind::Feature, Stage::Lld));
    assert!(feature.research.is_none());

    // The LLD's tasks are cards of the feature on the initiative's board.
    let feature = plan.submit(
        &feature,
        "docs/initiatives/combat/epics/t/lld.md",
        &["A", "B"],
    );
    let (feature, cards) = plan.approve(&feature, &[]);
    assert_eq!(feature.stage, Stage::Build);
    assert_eq!((feature.cards_total, feature.cards_done), (2, 0));
    for id in &cards {
        let item = plan.d.app.db.get_item(id).unwrap().unwrap();
        assert_eq!(item.item_type, ItemType::Task);
        assert_eq!(item.plan_node_id.as_deref(), Some(feature.id.as_str()));
        assert_eq!(Some(&item.board_id), init.board_id.as_ref());
        assert!(
            item.description.contains("in short"),
            "{}",
            item.description
        );
        // The summary goes under the template's first section, and the
        // card still has its Code research section to fill.
        assert!(
            item.description.starts_with("## What and why\n\n")
                && item.description.contains("## Code research"),
            "{}",
            item.description
        );
    }
    let kinds: Vec<String> = plan
        .owner()
        .detail(&feature.id)
        .unwrap()
        .unwrap()
        .history
        .into_iter()
        .map(|e| e.kind)
        .collect();
    assert_eq!(kinds, ["created", "submitted", "approved"]);
}

#[tokio::test]
async fn a_proposal_waits_for_the_owner_who_accepts_or_declines_it() {
    let plan = planned().await;
    let goal = plan.goal("Ship the demo");
    let propose = |title: &str| {
        done(plan.lead().propose(&c::PlanPropose {
            project_id: plan.pid.clone(),
            parent_id: goal.id.clone(),
            title: title.into(),
            summary: "Players can't read attacks.".into(),
        }))
    };
    let proposed = propose("Combat readability");
    assert_eq!(proposed.stage, Stage::Proposed);
    assert!(proposed.board_id.is_none() && proposed.research.is_none());
    let gate = proposed.gate.clone().expect("a gate");
    assert_eq!(gate.stage, Stage::Proposed);
    assert_eq!(gate.submitted_by, format!("bot:{}", plan.lead));
    assert_eq!(gate.note.as_deref(), Some("Players can't read attacks."));

    // Needs you asks the owner.
    let rows = flotillad::overview::attention_rows(&plan.d.app, &plan.pid).unwrap();
    let row = rows
        .rows
        .iter()
        .find(|r| r.kind == bus::contract::home::AttentionKind::PlanGate as i32)
        .expect("a plan gate row");
    assert_eq!(
        row.title,
        "Accept the proposed initiative Combat readability?"
    );
    assert_eq!(row.weight, 3);

    // Only the owner rules, and a bot is told so.
    let by_bot = plan.lead().rule(&c::PlanRule {
        id: proposed.id.clone(),
        expected_version: proposed.version,
        approve: true,
        ..Default::default()
    });
    assert_eq!(refused(by_bot), ["plan.owner_only"]);
    let (accepted, _) = plan.approve(&proposed, &[]);
    assert_eq!(accepted.stage, Stage::Research);
    assert_eq!(
        accepted.research.as_ref().unwrap().state,
        ResearchState::Open
    );
    assert!(accepted.board_id.is_some(), "accepted, it gets its board");
    let rows = flotillad::overview::attention_rows(&plan.d.app, &plan.pid).unwrap();
    assert!(!rows
        .rows
        .iter()
        .any(|r| r.kind == bus::contract::home::AttentionKind::PlanGate as i32));

    // Declining needs a note, and drops the proposal.
    let other = propose("Netcode");
    let rule = |note: Option<&str>| {
        plan.owner().rule(&c::PlanRule {
            id: other.id.clone(),
            expected_version: other.version,
            approve: false,
            note: note.map(str::to_string),
            ..Default::default()
        })
    };
    assert_eq!(refused(rule(None)), ["plan.note"]);
    let declined = done(rule(Some("Not this year")));
    assert_eq!(declined.stage, Stage::Dropped);
    assert!(declined.gate.is_none());
}
