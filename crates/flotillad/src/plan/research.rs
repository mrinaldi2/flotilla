//! A node's research (plan.proto): open until every spike of the node is
//! done with a document, an artifact link or an attachment. It is read
//! lazily, when the plan is read and when the lead submits: research that
//! closed is then recorded, its documents kept as the node's, and the node
//! moves on (an initiative to its PRD, an epic to its design). The owner can
//! also skip it, with a reason.
//!
//! With the project's research gate on (0.24.0, the default), research
//! whose spikes are done doesn't close by itself: it waits for the owner's
//! review of its findings (`awaiting`, a RESEARCH gate), and the owner's
//! approval closes it (`approve_findings`) or sends it back (`send_back`).

use super::model::{DocKind, Gate, Node, ResearchState, Stage};
use super::{changed, owner_only, Planner, Wrote};
use crate::db::{BoardTx, Spike};
use crate::decisions::not_found;
use crate::kanban::feed_by::DAEMON;
use crate::kanban::model::Unmet;

/// Why the node's research is still open, naming its spikes; `None` once
/// they close it.
pub(super) fn still_open(spikes: &[Spike]) -> Option<String> {
    if spikes.is_empty() {
        return Some("Research is open: the node has no spike yet.".to_string());
    }
    let waiting: Vec<String> = spikes
        .iter()
        .filter(|s| !s.done || s.docs.is_empty())
        .map(|s| match (s.done, s.docs.is_empty()) {
            (false, true) => format!("{} (not done, no document)", s.id),
            (false, false) => format!("{} (not done)", s.id),
            _ => format!("{} (no document)", s.id),
        })
        .collect();
    (!waiting.is_empty()).then(|| {
        format!(
            "Research is open: waiting on {}. A spike closes with a document linked or \
             attached, once it is done.",
            waiting.join(", ")
        )
    })
}

/// Whether `node` is researching: its research is open and it is in its
/// research stage.
fn researching(node: &Node) -> bool {
    node.stage == Stage::Research
        && node
            .research
            .as_ref()
            .is_some_and(|r| r.state == ResearchState::Open)
}

/// Close the node's research if its spikes now do; returns whether it is
/// closed after this. A node not researching is left as it is.
pub(super) fn close_if_done(
    t: &BoardTx<'_>,
    wrote: &mut Wrote,
    node: &Node,
) -> anyhow::Result<bool> {
    if !researching(node) {
        return Ok(false);
    }
    let spikes = t.plan_spikes(&node.id)?;
    if still_open(&spikes).is_some() {
        return Ok(false);
    }
    // A blocking question holds it open until the owner answers (0.22.0).
    if !t.blocking_questions(&node.id)?.is_empty() {
        return Ok(false);
    }
    for spike in &spikes {
        for doc in &spike.docs {
            t.upsert_plan_doc(&node.id, DocKind::Research, doc, Some(&spike.id))?;
        }
    }
    if t.plan_research_gate(&node.project_id)? {
        await_owner(t, wrote, node)?;
        return Ok(false);
    }
    close(t, wrote, node, DAEMON)?;
    Ok(true)
}

/// The findings wait for the owner: research `awaiting`, the node's gate
/// RESEARCH with its research documents (the node's), which replaces a
/// request to skip it.
fn await_owner(t: &BoardTx<'_>, wrote: &mut Wrote, node: &Node) -> anyhow::Result<()> {
    t.set_plan_research(&node.id, ResearchState::Awaiting, None)?;
    // Each review of the findings is a round, as each submission is.
    let rounds = t
        .plan_events(&node.id)?
        .iter()
        .filter(|e| e.kind == "research_awaiting")
        .count();
    let round = u32::try_from(rounds).unwrap_or(u32::MAX).saturating_add(1);
    t.set_plan_gate(
        &node.id,
        &Gate {
            stage: Stage::Research,
            doc_path: None,
            sha: None,
            pr_number: None,
            submitted_by: DAEMON.to_string(),
            submitted_at: bus::now(),
            note: None,
            children: Vec::new(),
            decision_id: None,
            round,
        },
    )?;
    let stage = Some(Stage::Research);
    changed(t, wrote, &node.id, DAEMON, "research_awaiting", stage, None)
}

/// Whether the node's research waits for the owner's findings review.
pub(super) fn awaiting(node: &Node) -> bool {
    node.research
        .as_ref()
        .is_some_and(|r| r.state == ResearchState::Awaiting)
}

/// The owner approves the findings: research closes and the node moves on.
pub(super) fn approve_findings(
    t: &BoardTx<'_>,
    wrote: &mut Wrote,
    node: &Node,
    by: &str,
    note: Option<&str>,
) -> anyhow::Result<()> {
    let stage = Some(Stage::Research);
    changed(t, wrote, &node.id, by, "approved", stage, note)?;
    close(t, wrote, node, by)
}

/// The owner sends the findings back: research is open again, and the
/// lead's next research spike carries the note (`kickoff`).
pub(super) fn send_back(
    t: &BoardTx<'_>,
    wrote: &mut Wrote,
    node: &Node,
    by: &str,
    note: &str,
) -> anyhow::Result<()> {
    t.set_plan_research(&node.id, ResearchState::Open, None)?;
    let stage = Some(Stage::Research);
    changed(t, wrote, &node.id, by, "sent_back", stage, Some(note))
}

/// Research closes and the node moves on to its next stage.
fn close(t: &BoardTx<'_>, wrote: &mut Wrote, node: &Node, by: &str) -> anyhow::Result<()> {
    t.set_plan_research(&node.id, ResearchState::Closed, None)?;
    // A request to skip it is moot now.
    if node
        .gate
        .as_ref()
        .is_some_and(|g| g.stage == Stage::Research)
    {
        t.clear_plan_gate(&node.id)?;
    }
    let next = node.kind.after_research();
    if let Some(next) = next {
        t.set_plan_stage(&node.id, next)?;
    }
    changed(t, wrote, &node.id, by, "research_closed", next, None)
}

/// Skip the node's research with the owner's reason and move it on.
pub(super) fn skip(
    t: &BoardTx<'_>,
    wrote: &mut Wrote,
    node: &Node,
    by: &str,
    reason: &str,
) -> anyhow::Result<()> {
    t.set_plan_research(&node.id, ResearchState::Skipped, Some(reason))?;
    let next = node.kind.after_research();
    if let Some(next) = next {
        t.set_plan_stage(&node.id, next)?;
    }
    changed(
        t,
        wrote,
        &node.id,
        by,
        "research_skipped",
        next,
        Some(reason),
    )
}

/// Record every node of the project whose spikes closed its research.
pub(super) fn settle(t: &BoardTx<'_>, wrote: &mut Wrote, project_id: &str) -> anyhow::Result<()> {
    for node in t.plan_nodes(project_id)?.iter().filter(|n| researching(n)) {
        close_if_done(t, wrote, node)?;
    }
    Ok(())
}

impl Planner<'_> {
    /// The owner turning the research gate on or off (0.24.0): on, research
    /// waits for the owner's review of its findings once its spikes are
    /// done; off, it closes by itself, as before.
    pub fn set_research_gate(&self, project_id: &str, on: bool) -> anyhow::Result<Option<Unmet>> {
        if !self.actor.is_owner() {
            return Ok(Some(owner_only()));
        }
        if !self
            .app
            .db
            .get_project(project_id)?
            .is_some_and(|p| p.deleted_at.is_none())
        {
            return Err(not_found(format!("no project {project_id}")));
        }
        self.app
            .db
            .board_tx(|t| t.set_plan_research_gate(project_id, on))?;
        Ok(None)
    }
}
