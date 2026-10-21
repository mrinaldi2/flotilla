//! The plan's own types (0.21.0, plan.proto): what `db::plan*` stores and
//! returns and what the plan service works with. Their stored spellings are
//! the SQL CHECK lists of migration 073; `plan::contract` maps them to the
//! wire.

use chrono::{DateTime, Utc};

use crate::kanban::model::{text_enum, TextValue};

text_enum!(
    /// What a node is: goal → initiative → epic → feature.
    Kind {
        Goal => "goal", Initiative => "initiative", Epic => "epic", Feature => "feature",
    }
);

text_enum!(
    /// Where a node is in its kind's stages; see plan.proto.
    Stage {
        Define => "define", Active => "active", Met => "met", Dropped => "dropped",
        Proposed => "proposed", Research => "research", Prd => "prd", Design => "design",
        Hld => "hld", Lld => "lld", Running => "running", Build => "build",
        Released => "released", Done => "done",
    }
);

text_enum!(
    /// A document's part in the plan. Guidelines are never stored: they are
    /// read from `docs/guidelines/` as they are.
    DocKind {
        Goal => "goal", Research => "research", Prd => "prd", Design => "design",
        Hld => "hld", Lld => "lld", Guideline => "guideline",
    }
);

text_enum!(ResearchState {
    Open => "open",
    Closed => "closed",
    Skipped => "skipped",
    // Every spike is done; the owner reviews the findings (0.24.0).
    Awaiting => "awaiting"
});

impl Kind {
    /// The kind of a node's parent; a goal has none.
    pub fn parent(self) -> Option<Kind> {
        match self {
            Kind::Goal => None,
            Kind::Initiative => Some(Kind::Goal),
            Kind::Epic => Some(Kind::Initiative),
            Kind::Feature => Some(Kind::Epic),
        }
    }

    /// Initiatives and epics research before their first gate.
    pub fn researches(self) -> bool {
        matches!(self, Kind::Initiative | Kind::Epic)
    }

    /// The stage research moves the node on to.
    pub fn after_research(self) -> Option<Stage> {
        match self {
            Kind::Initiative => Some(Stage::Prd),
            Kind::Epic => Some(Stage::Design),
            Kind::Goal | Kind::Feature => None,
        }
    }

    /// Whether `stage` is one of this kind's gates, where the lead submits a
    /// document for the owner.
    pub fn gates_at(self, stage: Stage) -> bool {
        matches!(
            (self, stage),
            (Kind::Initiative, Stage::Prd)
                | (Kind::Epic, Stage::Design | Stage::Hld)
                | (Kind::Feature, Stage::Lld)
        )
    }
}

impl Stage {
    /// The document a gate at this stage approves.
    pub fn doc_kind(self) -> Option<DocKind> {
        match self {
            Stage::Prd => Some(DocKind::Prd),
            Stage::Design => Some(DocKind::Design),
            Stage::Hld => Some(DocKind::Hld),
            Stage::Lld => Some(DocKind::Lld),
            _ => None,
        }
    }

    /// A stage nothing moves on from.
    pub fn is_final(self) -> bool {
        matches!(
            self,
            Stage::Met | Stage::Dropped | Stage::Released | Stage::Done
        )
    }
}

/// A document in the project's repository.
#[derive(Debug, Clone, PartialEq)]
pub struct Doc {
    pub kind: DocKind,
    pub path: String,
    pub approved_sha: Option<String>,
    pub approved_at: Option<DateTime<Utc>>,
    /// Computed in a node's detail only.
    pub changed_since_approval: bool,
    /// A research document's spike.
    pub item_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Research {
    pub state: ResearchState,
    pub spikes: Vec<String>,
    pub closed_at: Option<DateTime<Utc>>,
    pub skipped_reason: Option<String>,
}

/// A child a submission proposes: an epic, a feature or a task.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Child {
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

/// What waits for the owner's ruling on a node.
#[derive(Debug, Clone, PartialEq)]
pub struct Gate {
    pub stage: Stage,
    pub doc_path: Option<String>,
    pub sha: Option<String>,
    pub pr_number: Option<u32>,
    pub submitted_by: String,
    pub submitted_at: DateTime<Utc>,
    pub note: Option<String>,
    pub children: Vec<Child>,
    pub decision_id: Option<String>,
    pub round: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub id: String,
    pub project_id: String,
    pub kind: Kind,
    pub parent_id: Option<String>,
    pub title: String,
    pub summary: String,
    pub stage: Stage,
    pub due: Option<String>,
    pub lead: Option<String>,
    pub board_id: Option<String>,
    pub docs: Vec<Doc>,
    pub research: Option<Research>,
    pub gate: Option<Gate>,
    pub cards_total: u32,
    pub cards_done: u32,
    /// Open questions for the owner on it and its cards, and how many of
    /// them block it (0.22.0).
    pub questions_open: u32,
    pub questions_blocking: u32,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub version: u64,
    pub rank: String,
}

/// One line of a node's history. Its kind is text, so a newer daemon's
/// kinds still read.
#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    pub at: DateTime<Utc>,
    pub actor: String,
    pub kind: String,
    pub stage: Option<Stage>,
    pub note: Option<String>,
}

/// A project's planning switch and its node ids' sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settings {
    pub enabled: bool,
    pub next_seq: u32,
    /// Research waits for the owner's review of its findings (0.24.0).
    pub research_gate: bool,
}

/// A node to insert; the store gives it its id and rank.
pub struct NewNode<'a> {
    pub project_id: &'a str,
    pub kind: Kind,
    pub parent_id: Option<&'a str>,
    pub title: &'a str,
    pub summary: &'a str,
    pub stage: Stage,
    pub due: Option<&'a str>,
    pub lead: Option<&'a str>,
    pub board_id: Option<&'a str>,
    pub research: Option<ResearchState>,
}

/// Fields `plan_update` may change; `None` leaves one as it is and an
/// inner `None` clears it.
#[derive(Default)]
pub struct NodeEdit<'a> {
    pub title: Option<&'a str>,
    pub summary: Option<&'a str>,
    pub due: Option<Option<&'a str>>,
    pub lead: Option<Option<&'a str>>,
}
