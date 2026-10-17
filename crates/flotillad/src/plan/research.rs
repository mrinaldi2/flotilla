//! A node's research (plan.proto): open until every spike of the node is
//! done with a document, an artifact link or an attachment. It is read
//! lazily, when the plan is read and when the lead submits: research that
//! closed is then recorded, its documents kept as the node's, and the node
//! moves on (an initiative to its PRD, an epic to its design). The owner can
//! also skip it, with a reason.

use super::model::{DocKind, Node, ResearchState, Stage};
use super::{changed, Wrote};
use crate::db::{BoardTx, Spike};
use crate::kanban::feed_by::DAEMON;

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
    for spike in &spikes {
        for doc in &spike.docs {
            t.upsert_plan_doc(&node.id, DocKind::Research, doc, Some(&spike.id))?;
        }
    }
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
    changed(t, wrote, &node.id, DAEMON, "research_closed", next, None)?;
    Ok(true)
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
