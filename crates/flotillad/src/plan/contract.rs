//! The plan's model and the wire contract (plan.proto): model → contract is
//! infallible, contract → model is checked, as in `kanban::contract`. The
//! enum matches are exhaustive both ways, so a stage added on either side
//! fails to compile until it is mapped.

use bus::contract::board as c;

use super::model as m;
use crate::kanban::contract::{all, at, maybe_at, stamp, MapError};

/// One model enum and its contract enum, variant by variant.
macro_rules! plan_enum {
    ($model:ident, $wire:ident { $($variant:ident),+ $(,)? }) => {
        impl From<m::$model> for c::$wire {
            fn from(value: m::$model) -> Self {
                match value {
                    $(m::$model::$variant => c::$wire::$variant),+
                }
            }
        }

        impl From<m::$model> for i32 {
            fn from(value: m::$model) -> Self {
                c::$wire::from(value) as i32
            }
        }

        impl m::$model {
            /// From the number a message field carries.
            pub fn from_wire(number: i32) -> Result<Self, MapError> {
                match c::$wire::try_from(number) {
                    $(Ok(c::$wire::$variant) => Ok(m::$model::$variant),)+
                    Ok(c::$wire::Unspecified) | Err(_) => {
                        Err(MapError::UnknownEnum(stringify!($wire)))
                    }
                }
            }
        }
    };
}

plan_enum!(
    Kind,
    PlanKind {
        Goal,
        Initiative,
        Epic,
        Feature
    }
);
plan_enum!(
    Stage,
    PlanStage {
        Define,
        Active,
        Met,
        Dropped,
        Proposed,
        Research,
        Prd,
        Design,
        Hld,
        Lld,
        Running,
        Build,
        Released,
        Done
    }
);
plan_enum!(
    DocKind,
    PlanDocKind {
        Goal,
        Research,
        Prd,
        Design,
        Hld,
        Lld,
        Guideline
    }
);
plan_enum!(
    ResearchState,
    PlanResearchState {
        Open,
        Closed,
        Skipped
    }
);

/// Every plan enum's short and wire names, for bots' output (MCP writes
/// the short ones, as for the board's enums).
pub fn spellings() -> Vec<(&'static str, &'static str)> {
    let mut out: Vec<(&'static str, &'static str)> = Vec::new();
    out.extend(
        m::Kind::ALL
            .iter()
            .map(|v| (v.as_str(), c::PlanKind::from(*v).as_str_name())),
    );
    out.extend(
        m::Stage::ALL
            .iter()
            .map(|v| (v.as_str(), c::PlanStage::from(*v).as_str_name())),
    );
    out.extend(
        m::DocKind::ALL
            .iter()
            .map(|v| (v.as_str(), c::PlanDocKind::from(*v).as_str_name())),
    );
    out.extend(
        m::ResearchState::ALL
            .iter()
            .map(|v| (v.as_str(), c::PlanResearchState::from(*v).as_str_name())),
    );
    out
}

impl From<m::Doc> for c::PlanDoc {
    fn from(v: m::Doc) -> Self {
        c::PlanDoc {
            kind: v.kind.into(),
            path: v.path,
            approved_sha: v.approved_sha,
            approved_at: v.approved_at.and_then(stamp),
            changed_since_approval: v.changed_since_approval,
            item_id: v.item_id,
        }
    }
}

impl From<m::Research> for c::PlanResearch {
    fn from(v: m::Research) -> Self {
        c::PlanResearch {
            state: v.state.into(),
            spikes: v.spikes,
            closed_at: v.closed_at.and_then(stamp),
            skipped_reason: v.skipped_reason,
        }
    }
}

impl From<m::Child> for c::PlanChild {
    fn from(v: m::Child) -> Self {
        c::PlanChild {
            title: v.title,
            summary: v.summary,
        }
    }
}

impl From<c::PlanChild> for m::Child {
    fn from(v: c::PlanChild) -> Self {
        m::Child {
            title: v.title,
            summary: v.summary,
        }
    }
}

impl From<m::Gate> for c::PlanGate {
    fn from(v: m::Gate) -> Self {
        c::PlanGate {
            stage: v.stage.into(),
            doc_path: v.doc_path,
            sha: v.sha,
            pr_number: v.pr_number,
            submitted_by: v.submitted_by,
            submitted_at: stamp(v.submitted_at),
            note: v.note,
            children: v.children.into_iter().map(Into::into).collect(),
            decision_id: v.decision_id,
            round: v.round,
        }
    }
}

impl From<m::Node> for c::PlanNode {
    fn from(v: m::Node) -> Self {
        c::PlanNode {
            id: v.id,
            project_id: v.project_id,
            kind: v.kind.into(),
            parent_id: v.parent_id,
            title: v.title,
            summary: v.summary,
            stage: v.stage.into(),
            due: v.due,
            lead: v.lead,
            board_id: v.board_id,
            docs: v.docs.into_iter().map(Into::into).collect(),
            research: v.research.map(Into::into),
            gate: v.gate.map(Into::into),
            cards_total: v.cards_total,
            cards_done: v.cards_done,
            created_by: v.created_by,
            created_at: stamp(v.created_at),
            updated_at: stamp(v.updated_at),
            version: v.version,
            rank: v.rank,
            questions_open: v.questions_open,
            questions_blocking: v.questions_blocking,
        }
    }
}

impl From<m::Event> for c::PlanEvent {
    fn from(v: m::Event) -> Self {
        c::PlanEvent {
            at: stamp(v.at),
            actor: v.actor,
            kind: v.kind,
            stage: v.stage.map(Into::into),
            note: v.note,
        }
    }
}

impl TryFrom<c::PlanDoc> for m::Doc {
    type Error = MapError;

    fn try_from(v: c::PlanDoc) -> Result<Self, MapError> {
        Ok(m::Doc {
            kind: m::DocKind::from_wire(v.kind)?,
            path: v.path,
            approved_sha: v.approved_sha,
            approved_at: maybe_at(v.approved_at, "PlanDoc.approved_at")?,
            changed_since_approval: v.changed_since_approval,
            item_id: v.item_id,
        })
    }
}

impl TryFrom<c::PlanResearch> for m::Research {
    type Error = MapError;

    fn try_from(v: c::PlanResearch) -> Result<Self, MapError> {
        Ok(m::Research {
            state: m::ResearchState::from_wire(v.state)?,
            spikes: v.spikes,
            closed_at: maybe_at(v.closed_at, "PlanResearch.closed_at")?,
            skipped_reason: v.skipped_reason,
        })
    }
}

impl TryFrom<c::PlanGate> for m::Gate {
    type Error = MapError;

    fn try_from(v: c::PlanGate) -> Result<Self, MapError> {
        Ok(m::Gate {
            stage: m::Stage::from_wire(v.stage)?,
            doc_path: v.doc_path,
            sha: v.sha,
            pr_number: v.pr_number,
            submitted_by: v.submitted_by,
            submitted_at: at(v.submitted_at, "PlanGate.submitted_at")?,
            note: v.note,
            children: v.children.into_iter().map(Into::into).collect(),
            decision_id: v.decision_id,
            round: v.round,
        })
    }
}

impl TryFrom<c::PlanNode> for m::Node {
    type Error = MapError;

    fn try_from(v: c::PlanNode) -> Result<Self, MapError> {
        Ok(m::Node {
            id: v.id,
            project_id: v.project_id,
            kind: m::Kind::from_wire(v.kind)?,
            parent_id: v.parent_id,
            title: v.title,
            summary: v.summary,
            stage: m::Stage::from_wire(v.stage)?,
            due: v.due,
            lead: v.lead,
            board_id: v.board_id,
            docs: all(v.docs)?,
            research: v.research.map(TryInto::try_into).transpose()?,
            gate: v.gate.map(TryInto::try_into).transpose()?,
            cards_total: v.cards_total,
            cards_done: v.cards_done,
            questions_open: v.questions_open,
            questions_blocking: v.questions_blocking,
            created_by: v.created_by,
            created_at: at(v.created_at, "PlanNode.created_at")?,
            updated_at: at(v.updated_at, "PlanNode.updated_at")?,
            version: v.version,
            rank: v.rank,
        })
    }
}

impl From<super::Tree> for c::PlanTree {
    fn from(v: super::Tree) -> Self {
        c::PlanTree {
            project_id: v.project_id,
            enabled: v.enabled,
            has_repo: v.has_repo,
            nodes: v.nodes.into_iter().map(Into::into).collect(),
            guidelines: v.guidelines.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<super::Detail> for c::PlanNodeDetail {
    fn from(v: super::Detail) -> Self {
        c::PlanNodeDetail {
            node: Some(v.node.into()),
            history: v.history.into_iter().map(Into::into).collect(),
            cards: v.cards.into_iter().map(Into::into).collect(),
            questions: v.questions,
        }
    }
}

impl From<super::Planned> for c::PlanResult {
    fn from(v: super::Planned) -> Self {
        use c::plan_result::Outcome;
        c::PlanResult {
            outcome: Some(match v.outcome {
                super::Outcome::Done(node) => Outcome::Done((*node).into()),
                super::Outcome::Refused(unmet) => Outcome::Refused(c::PlanRefused {
                    unmet: unmet.into_iter().map(Into::into).collect(),
                }),
                super::Outcome::Conflict(node) => Outcome::Conflict((*node).into()),
            }),
            created: v.created,
        }
    }
}

#[cfg(test)]
#[path = "contract_tests.rs"]
mod tests;
