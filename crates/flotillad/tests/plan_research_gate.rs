//! The research gate (0.24.0, `research_gate_v1`): on by default, research
//! whose spikes are done waits for the owner's review of its findings. The
//! owner approves them (research closes, the next gate opens) or sends them
//! back with a note (research reopens, the lead gets a spike with it). Off,
//! research closes by itself as before.

use crate::common;

use bus::contract::board as c;
use bus::contract::home::AttentionKind;
use common::plan::*;
use flotillad::kanban::model::ItemType;
use flotillad::plan::model::{Node, ResearchState, Stage};

/// The node as the plan reads it now: research its spikes closed recorded.
fn read(plan: &Plan, id: &str) -> Node {
    let tree = plan.owner().tree(&plan.pid).unwrap();
    tree.nodes
        .into_iter()
        .find(|n| n.id == id)
        .expect("the node")
}

fn research(node: &Node) -> ResearchState {
    node.research.as_ref().expect("research").state
}

fn history(plan: &Plan, id: &str) -> Vec<String> {
    let detail = plan.owner().detail(id).unwrap().unwrap();
    detail.history.into_iter().map(|e| e.kind).collect()
}

/// An initiative whose spikes are all done with their documents.
fn researched(plan: &Plan) -> Node {
    let goal = plan.goal("Ship the demo");
    let init = plan.initiative(&goal.id, "Combat readability");
    let spike = plan.spike(&init, "Look at other games");
    plan.finish(&spike);
    plan.document(&spike, "research/references.md");
    plan.research_kickoff_done(&init.id);
    read(plan, &init.id)
}

fn rule(plan: &Plan, node: &Node, approve: bool, note: Option<&str>) -> flotillad::plan::Planned {
    plan.owner()
        .rule(&c::PlanRule {
            id: node.id.clone(),
            expected_version: node.version,
            approve,
            note: note.map(str::to_string),
            ..Default::default()
        })
        .expect("rule")
}

fn gate_row_title(plan: &Plan) -> Option<String> {
    let rows = flotillad::overview::attention_rows(&plan.d.app, &plan.pid).unwrap();
    rows.rows
        .into_iter()
        .find(|r| r.kind == AttentionKind::PlanGate as i32)
        .map(|r| r.title)
}

#[tokio::test]
async fn finished_research_waits_for_the_owners_review_of_its_findings() {
    let plan = gated().await;
    assert!(
        plan.owner().tree(&plan.pid).unwrap().research_gate,
        "on by default"
    );
    let init = researched(&plan);
    assert_eq!(research(&init), ResearchState::Awaiting);
    assert_eq!(init.stage, Stage::Research, "it waits where it is");
    let gate = init.gate.clone().expect("a RESEARCH gate");
    assert_eq!(gate.stage, Stage::Research);
    assert!(
        init.docs.iter().any(|d| d.path == "research/references.md"),
        "with its research documents"
    );
    assert_eq!(
        gate_row_title(&plan).as_deref(),
        Some("Approve the research findings of Combat readability?")
    );
    assert!(history(&plan, &init.id).contains(&"research_awaiting".to_string()));

    // The lead can't write the PRD past it.
    let early = plan.lead().submit(&c::PlanSubmit {
        id: init.id.clone(),
        expected_version: init.version,
        doc_path: "docs/initiatives/combat/prd.md".into(),
        ..Default::default()
    });
    assert_eq!(refused(early), ["plan.research_awaiting"]);
    // Nor rule on it: it's the owner's.
    let by_lead = plan.lead().rule(&c::PlanRule {
        id: init.id.clone(),
        expected_version: init.version,
        approve: true,
        ..Default::default()
    });
    assert_eq!(refused(by_lead), ["plan.owner_only"]);

    // Approved, research closes and the PRD is next.
    let approved = done(Ok(rule(&plan, &init, true, None)));
    assert_eq!(research(&approved), ResearchState::Closed);
    assert_eq!(approved.stage, Stage::Prd);
    assert!(approved.gate.is_none());
    let kinds = history(&plan, &init.id);
    assert!(
        kinds.ends_with(&["approved".to_string(), "research_closed".to_string()]),
        "{kinds:?}"
    );
    assert!(plan.kickoff(&init.id, "Write the PRD: ").is_some());
    assert_eq!(gate_row_title(&plan), None);
}

#[tokio::test]
async fn sent_back_findings_reopen_research_with_the_owners_note() {
    let plan = gated().await;
    let init = researched(&plan);
    // A note is required: the lead reads it.
    let silent = rule(&plan, &init, false, None);
    assert_eq!(refused(Ok(silent)), ["plan.note"]);
    let back = done(Ok(rule(
        &plan,
        &init,
        false,
        Some("Look at fighting games too."),
    )));
    assert_eq!(research(&back), ResearchState::Open);
    assert!(back.gate.is_none());
    assert_eq!(back.stage, Stage::Research);

    // The lead gets a spike carrying the note; research waits on it.
    let revise = plan
        .kickoff(&init.id, "Revise the research")
        .expect("a revision spike");
    let item = plan.d.app.db.get_item(&revise).unwrap().unwrap();
    assert_eq!(item.item_type, ItemType::Spike);
    assert!(
        item.description.contains("Look at fighting games too."),
        "{}",
        item.description
    );
    assert_eq!(item.assignee.as_deref(), Some(plan.lead.as_str()));
    assert_eq!(research(&read(&plan, &init.id)), ResearchState::Open);

    // Done with its document, the findings go back to the owner.
    plan.finish(&revise);
    plan.document(&revise, "research/fighting-games.md");
    let again = read(&plan, &init.id);
    assert_eq!(research(&again), ResearchState::Awaiting);
    assert_eq!(again.gate.as_ref().map(|g| g.round), Some(2));
    let approved = done(Ok(rule(&plan, &again, true, None)));
    assert_eq!(approved.stage, Stage::Prd);
}

#[tokio::test]
async fn turned_off_research_closes_by_itself() {
    let plan = gated().await;
    assert!(plan
        .owner()
        .set_research_gate(&plan.pid, false)
        .unwrap()
        .is_none());
    assert!(!plan.owner().tree(&plan.pid).unwrap().research_gate);
    // Only the owner turns it.
    let by_lead = plan.lead().set_research_gate(&plan.pid, true).unwrap();
    assert_eq!(by_lead.expect("refused").code, "plan.owner_only");
    let init = researched(&plan);
    assert_eq!(research(&init), ResearchState::Closed);
    assert_eq!(init.stage, Stage::Prd);
    assert!(init.gate.is_none());
    assert_eq!(gate_row_title(&plan), None);
}

#[tokio::test]
async fn plan_enable_sets_it_and_the_tree_reads_it() {
    use c::{board_request::Request, board_response::Response};
    use common::board::{call, response};
    let mut plan = unplanned().await;
    let enable = |research_gate: Option<bool>| {
        Request::PlanEnable(c::PlanEnable {
            project_id: plan.pid.clone(),
            enabled: true,
            research_gate,
        })
    };
    let (on, off, kept) = (enable(None), enable(Some(false)), enable(None));
    let tree = |r: Response| match r {
        Response::Plan(tree) => tree,
        other => panic!("a tree: {other:?}"),
    };
    assert!(tree(response(call(&mut plan.owner, on).await)).research_gate);
    assert!(!tree(response(call(&mut plan.owner, off).await)).research_gate);
    // Left out, it stays as it is.
    assert!(!tree(response(call(&mut plan.owner, kept).await)).research_gate);
}
