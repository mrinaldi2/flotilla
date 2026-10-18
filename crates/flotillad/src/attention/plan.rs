//! The plan's gates (0.21.0): one row per node waiting for the owner's
//! ruling, built on the board's home where the plan lives.

use bus::contract::home::{attention_row::Target, AttentionKind};

use super::rows::{Builder, Part};
use super::weight;
use crate::app::AppState;
use crate::plan::model::{Node, Stage};

/// What the owner is asked, in their words.
fn question(node: &Node, stage: Stage) -> String {
    let title = &node.title;
    match stage {
        Stage::Proposed => format!("Accept the proposed initiative {title}?"),
        Stage::Research => format!("Skip research for {title}?"),
        Stage::Prd => format!("Approve the PRD of {title}?"),
        Stage::Design => format!("Approve the design of {title}?"),
        Stage::Hld => format!("Approve the HLD of {title}?"),
        Stage::Lld => format!("Approve the LLD of {title}?"),
        other => format!("Rule on {title} ({})?", other.as_str()),
    }
}

pub(super) fn gates(app: &AppState, b: &mut Builder<'_>) -> anyhow::Result<()> {
    let project_id = b.project_id;
    let nodes = app.db.board_read(|t| {
        if !t.plan_settings(project_id)?.is_some_and(|s| s.enabled) {
            return Ok(Vec::new());
        }
        t.plan_nodes(project_id)
    })?;
    for node in nodes {
        let Some(gate) = node.gate.as_ref() else {
            continue;
        };
        let part = Part {
            kind: AttentionKind::PlanGate,
            target_id: node.id.clone(),
            title: question(&node, gate.stage),
            created_at: gate.submitted_at,
            target: Some(Target::PlanNodeId(node.id.clone())),
        };
        b.push(part, weight(AttentionKind::PlanGate), None);
    }
    Ok(())
}
