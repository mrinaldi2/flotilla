//! What a new board starts from (0.19.0 K3): data, not code paths. Simple,
//! Simple + Review and Software are custom boards ruled by their columns;
//! Release-gated is the flow every board had before 0.19. Everything can be
//! changed afterwards. A project's first board is Simple + Review.

use std::collections::BTreeMap;

use super::defaults::COLUMNS;
use super::facts::Fact;
use super::flow::{ColumnPolicy, EntryRule, Mover};
use super::model::{BoardFlow, ColumnCategory, ColumnKind, Role, Size, WipScope};

mod start;
#[cfg(test)]
mod start_tests;

pub use start::Start;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
    Simple,
    /// A project's first board: the bot works a card and moves it to
    /// Review; the Tester (or the owner) checks it and moves it to Done.
    SimpleReview,
    Software,
    ReleaseGated,
}

#[derive(Debug, Clone)]
pub struct PresetColumn {
    pub key: String,
    pub name: String,
    pub kind: ColumnKind,
    pub wip_limit: Option<u32>,
    pub wip_scope: WipScope,
    pub visible: bool,
    pub policy: ColumnPolicy,
    /// The Research column (0.21.0): an active column whose category is
    /// `research`, where a task or bug's assignee reads the code first.
    pub research: bool,
    /// The role a card entering it is assigned to (0.24.0): the default
    /// chain on Simple + Review and Software, none elsewhere.
    pub on_entry: Option<Role>,
}

/// The category a custom column carries for the code that still reads
/// categories (the inbox a new item lands in, metrics): its kind's.
pub fn category_of(kind: ColumnKind) -> ColumnCategory {
    match kind {
        ColumnKind::Open => ColumnCategory::Inbox,
        ColumnKind::Active => ColumnCategory::Doing,
        ColumnKind::Done => ColumnCategory::Done,
        ColumnKind::Cancelled => ColumnCategory::Cancelled,
    }
}

impl PresetColumn {
    pub fn category(&self) -> ColumnCategory {
        match self.research && self.kind == ColumnKind::Active {
            true => ColumnCategory::Research,
            false => category_of(self.kind),
        }
    }
}

impl Preset {
    pub const ALL: &'static [Preset] = &[
        Preset::Simple,
        Preset::SimpleReview,
        Preset::Software,
        Preset::ReleaseGated,
    ];

    /// What a project's first board is made from.
    pub const FIRST: Preset = Preset::SimpleReview;

    pub fn name(self) -> &'static str {
        match self {
            Preset::Simple => "simple",
            Preset::SimpleReview => "simple_review",
            Preset::Software => "software",
            Preset::ReleaseGated => "release_gated",
        }
    }

    pub fn parse(name: &str) -> Option<Preset> {
        Preset::ALL.iter().copied().find(|p| p.name() == name)
    }

    pub fn flow(self) -> BoardFlow {
        match self {
            Preset::ReleaseGated => BoardFlow::ReleaseGated,
            Preset::Simple | Preset::SimpleReview | Preset::Software => BoardFlow::Custom,
        }
    }

    pub fn columns(self) -> Vec<PresetColumn> {
        let mut columns = self.plain_columns();
        if matches!(self, Preset::SimpleReview | Preset::Software) {
            for column in &mut columns {
                column.on_entry = chain(&column.key);
                if let Some(role) = chain_mover(&column.key) {
                    column
                        .policy
                        .movers
                        .push(Mover::Roles(vec![role.as_str().into()]));
                }
            }
        }
        columns
    }

    fn plain_columns(self) -> Vec<PresetColumn> {
        match self {
            Preset::Simple => vec![
                col(
                    "todo",
                    "To do",
                    ColumnKind::Open,
                    None,
                    ColumnPolicy::default(),
                ),
                research(ColumnPolicy::default()),
                col(
                    "doing",
                    "Doing",
                    ColumnKind::Active,
                    Some(3),
                    ColumnPolicy {
                        rules: vec![EntryRule::ResearchRecorded],
                        ..ColumnPolicy::default()
                    },
                ),
                col(
                    "done",
                    "Done",
                    ColumnKind::Done,
                    None,
                    ColumnPolicy::default(),
                ),
                cancelled(),
            ],
            Preset::SimpleReview => simple_review(),
            Preset::Software => vec![
                col(
                    "inbox",
                    "Inbox",
                    ColumnKind::Open,
                    None,
                    ColumnPolicy::default(),
                ),
                col(
                    "ready",
                    "Ready",
                    ColumnKind::Open,
                    Some(10),
                    ColumnPolicy {
                        movers: vec![Mover::Lead],
                        rules: vec![
                            EntryRule::AcPresent,
                            EntryRule::SizeAtMost { size: Size::M },
                            EntryRule::BlockersDone,
                        ],
                        allowed_from: Vec::new(),
                    },
                ),
                research(ColumnPolicy {
                    movers: vec![Mover::Lead, Mover::Assignee],
                    rules: vec![EntryRule::AssigneeSet],
                    allowed_from: Vec::new(),
                }),
                PresetColumn {
                    wip_scope: WipScope::PerAssignee,
                    ..col(
                        "doing",
                        "Doing",
                        ColumnKind::Active,
                        Some(1),
                        ColumnPolicy {
                            movers: vec![Mover::Lead, Mover::Assignee],
                            rules: vec![EntryRule::AssigneeSet, EntryRule::ResearchRecorded],
                            allowed_from: Vec::new(),
                        },
                    )
                },
                col(
                    "review",
                    "Review",
                    ColumnKind::Active,
                    Some(3),
                    ColumnPolicy {
                        movers: vec![Mover::Assignee],
                        rules: vec![EntryRule::Link { kind: "pr".into() }],
                        allowed_from: vec!["doing".into()],
                    },
                ),
                col(
                    "done",
                    "Done",
                    ColumnKind::Done,
                    None,
                    ColumnPolicy {
                        movers: vec![Mover::Lead],
                        rules: Vec::new(),
                        allowed_from: Vec::new(),
                    },
                ),
                cancelled(),
            ],
            Preset::ReleaseGated => COLUMNS
                .iter()
                .map(|c| PresetColumn {
                    key: c.key.to_string(),
                    name: c.name.to_string(),
                    kind: c.category.kind(),
                    wip_limit: c.wip_limit,
                    wip_scope: c.wip_scope,
                    visible: c.visible,
                    policy: ColumnPolicy::default(),
                    research: c.category == ColumnCategory::Research,
                    on_entry: None,
                })
                .collect(),
        }
    }

    /// The category each column has: a release-gated board keeps the
    /// categories its guards key on.
    pub fn categories(self) -> Vec<ColumnCategory> {
        match self {
            Preset::ReleaseGated => COLUMNS.iter().map(|c| c.category).collect(),
            _ => self.columns().iter().map(PresetColumn::category).collect(),
        }
    }

    /// Fact → column key, for custom boards.
    pub fn automations(self) -> BTreeMap<String, String> {
        let pairs: &[(Fact, &str)] = match self {
            Preset::Software => &[
                (Fact::PrOpened, "review"),
                (Fact::PrMerged, "done"),
                (Fact::PrClosed, "doing"),
                (Fact::PrChangesRequested, "doing"),
            ],
            // Starting a card is reading its code first (0.21.0).
            Preset::SimpleReview => &[(Fact::WorkStarted, "research")],
            Preset::Simple | Preset::ReleaseGated => &[],
        };
        pairs
            .iter()
            .map(|(f, c)| (f.name().to_string(), (*c).to_string()))
            .collect()
    }
}

/// The default chain (0.24.0): the role a column of Simple + Review or
/// Software assigns a card entering it, by its key. Each role moves the card
/// on to the next column, which assigns the next role; the Tester closes a
/// passed card in Done, which its movers then list.
pub fn chain(key: &str) -> Option<Role> {
    Some(match key {
        "todo" | "inbox" => Role::Product,
        "research" => Role::Architect,
        "doing" => Role::Dev,
        "review" | "verify" => Role::Tester,
        _ => return None,
    })
}

/// The role the chain adds to a column's movers, by its key, so the role
/// before it can move a card on: the Tester closes a passed card in Done,
/// and Software's Product moves a shaped card to Ready.
pub fn chain_mover(key: &str) -> Option<Role> {
    Some(match key {
        "done" => Role::Tester,
        "ready" => Role::Product,
        _ => return None,
    })
}

fn col(
    key: &'static str,
    name: &'static str,
    kind: ColumnKind,
    wip_limit: Option<u32>,
    policy: ColumnPolicy,
) -> PresetColumn {
    PresetColumn {
        key: key.to_string(),
        name: name.to_string(),
        kind,
        wip_limit,
        wip_scope: WipScope::Column,
        visible: true,
        policy,
        research: false,
        on_entry: None,
    }
}

/// Between the open columns and Doing: the assignee reads the code and
/// records what it found before the card may enter Doing (0.21.0).
fn research(policy: ColumnPolicy) -> PresetColumn {
    PresetColumn {
        research: true,
        ..col("research", "Research", ColumnKind::Active, None, policy)
    }
}

/// To do → Research → Doing → Review → Done. The assignee moves its own card
/// along and Doing → Review; the daemon moves it to Research when the
/// assignee starts (`work.started`), and a task or bug enters Doing once its
/// code research is recorded. Done is the owner's, or the lead's when the
/// lead isn't the card's assignee, so a bot never approves its own work.
fn simple_review() -> Vec<PresetColumn> {
    let assignee_or_lead = || vec![Mover::Lead, Mover::Assignee];
    vec![
        col(
            "todo",
            "To do",
            ColumnKind::Open,
            None,
            ColumnPolicy {
                movers: assignee_or_lead(),
                rules: Vec::new(),
                allowed_from: Vec::new(),
            },
        ),
        research(ColumnPolicy {
            movers: assignee_or_lead(),
            rules: vec![EntryRule::AssigneeSet],
            allowed_from: Vec::new(),
        }),
        col(
            "doing",
            "Doing",
            ColumnKind::Active,
            Some(3),
            ColumnPolicy {
                movers: assignee_or_lead(),
                rules: vec![EntryRule::AssigneeSet, EntryRule::ResearchRecorded],
                allowed_from: Vec::new(),
            },
        ),
        col(
            "review",
            "Review",
            ColumnKind::Active,
            None,
            ColumnPolicy {
                movers: assignee_or_lead(),
                rules: Vec::new(),
                allowed_from: vec!["doing".into()],
            },
        ),
        col(
            "done",
            "Done",
            ColumnKind::Done,
            None,
            ColumnPolicy {
                movers: vec![Mover::Lead],
                rules: vec![EntryRule::Independent],
                allowed_from: Vec::new(),
            },
        ),
        cancelled(),
    ]
}

fn cancelled() -> PresetColumn {
    PresetColumn {
        visible: false,
        ..col(
            "cancelled",
            "Cancelled",
            ColumnKind::Cancelled,
            None,
            ColumnPolicy {
                movers: vec![Mover::Lead],
                rules: vec![EntryRule::Reason],
                allowed_from: Vec::new(),
            },
        )
    }
}
