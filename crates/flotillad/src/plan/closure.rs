//! Nodes closing as their work ships (0.21.0, #43): a feature in build is
//! released once every card of it is done (a cancelled card aside); an epic
//! that is running is done once every feature of it is released, and an
//! initiative once every epic of it is done (dropped ones aside, at least
//! one left). Like research, it is settled when the plan is read and when a
//! card of the plan reaches Done (`plan::watch`), as the daemon's change.

use super::model::{Kind, Node, Stage};
use super::{changed, Wrote};
use crate::db::BoardTx;
use crate::kanban::feed_by::DAEMON;

/// The stage a finished child of `kind` is in, and the kind of child.
fn finished_child(kind: Kind) -> Option<(Kind, Stage)> {
    match kind {
        Kind::Epic => Some((Kind::Feature, Stage::Released)),
        Kind::Initiative => Some((Kind::Epic, Stage::Done)),
        Kind::Goal | Kind::Feature => None,
    }
}

fn close(t: &BoardTx<'_>, wrote: &mut Wrote, node: &Node, stage: Stage) -> anyhow::Result<()> {
    t.set_plan_stage(&node.id, stage)?;
    let kind = match stage {
        Stage::Released => "released",
        _ => "done",
    };
    changed(t, wrote, &node.id, DAEMON, kind, Some(stage), None)
}

/// Close every node of the project whose work has shipped, features first
/// so their epics and initiatives follow in the same pass.
pub(super) fn settle(t: &BoardTx<'_>, wrote: &mut Wrote, project_id: &str) -> anyhow::Result<()> {
    for feature in t
        .plan_nodes(project_id)?
        .iter()
        .filter(|n| n.kind == Kind::Feature && n.stage == Stage::Build)
    {
        if feature.cards_total > 0 && feature.cards_done == feature.cards_total {
            close(t, wrote, feature, Stage::Released)?;
        }
    }
    for kind in [Kind::Epic, Kind::Initiative] {
        let nodes = t.plan_nodes(project_id)?;
        let (child_kind, finished) = finished_child(kind).unwrap_or((kind, Stage::Done));
        for node in nodes
            .iter()
            .filter(|n| n.kind == kind && n.stage == Stage::Running)
        {
            let children: Vec<&Node> = nodes
                .iter()
                .filter(|c| {
                    c.parent_id.as_deref() == Some(node.id.as_str())
                        && c.kind == child_kind
                        && c.stage != Stage::Dropped
                })
                .collect();
            if !children.is_empty() && children.iter().all(|c| c.stage == finished) {
                close(t, wrote, node, Stage::Done)?;
            }
        }
    }
    Ok(())
}
