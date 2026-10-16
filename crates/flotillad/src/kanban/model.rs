//! The board's own types: what the repository stores and returns and what the
//! service layer works with. The wire contract has its own generated types;
//! `kanban::contract` maps between the two, so a change of contract codegen
//! (protobuf since ruling dcf069e2) touches only that mapping.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};

/// A closed value set stored as text.
pub trait TextValue: Sized + Copy {
    fn as_text(self) -> &'static str;
    fn from_text(text: &str) -> Option<Self>;
}

/// A closed value set with a stable stored spelling.
macro_rules! text_enum {
    ($(#[$doc:meta])* $name:ident { $($variant:ident => $text:literal),+ $(,)? }) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum $name {
            $($variant),+
        }

        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$variant),+];

            /// How it is stored, which is also its wire spelling today.
            pub fn as_str(self) -> &'static str {
                match self {
                    $($name::$variant => $text),+
                }
            }

            pub fn parse(text: &str) -> Option<Self> {
                match text {
                    $($text => Some($name::$variant),)+
                    _ => None,
                }
            }
        }

        impl serde::Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.serialize_str(self.as_str())
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let text = String::deserialize(d)?;
                Self::parse(&text).ok_or_else(|| {
                    serde::de::Error::custom(format!("unknown {}: {text}", stringify!($name)))
                })
            }
        }

        impl TextValue for $name {
            fn as_text(self) -> &'static str {
                self.as_str()
            }

            fn from_text(text: &str) -> Option<Self> {
                Self::parse(text)
            }
        }
    };
}
pub(crate) use text_enum;

text_enum!(
    /// The canonical category a column maps to; guards and metrics key on it.
    ColumnCategory {
        Inbox => "inbox", Ready => "ready", Doing => "doing", Review => "review",
        Verify => "verify", Approval => "approval", Deploying => "deploying",
        Done => "done", Cancelled => "cancelled",
    }
);
text_enum!(
    /// What a column means to the board itself (0.19.0 K2): WIP, staleness
    /// and cycle time apply to `active` columns; `done` and `cancelled` end.
    ColumnKind { Open => "open", Active => "active", Done => "done", Cancelled => "cancelled" }
);
text_enum!(
    /// How a board decides moves (0.19.0 K3): the release-gated flow every
    /// board had before 0.19, or its columns' own movers and rules.
    BoardFlow { ReleaseGated => "release_gated", Custom => "custom" }
);
text_enum!(
    /// Who may change a board's columns, rules and types besides the owner.
    StructurePolicy { OwnerConfirms => "owner_confirms", LeadFree => "lead_free" }
);
text_enum!(WipScope { Column => "column", PerAssignee => "per_assignee" });

impl ColumnCategory {
    /// The kind a column of this category has: until K3 replaces categories
    /// with rules, the kind follows the category.
    pub fn kind(self) -> ColumnKind {
        match self {
            ColumnCategory::Inbox | ColumnCategory::Ready => ColumnKind::Open,
            ColumnCategory::Done => ColumnKind::Done,
            ColumnCategory::Cancelled => ColumnKind::Cancelled,
            ColumnCategory::Doing
            | ColumnCategory::Review
            | ColumnCategory::Verify
            | ColumnCategory::Approval
            | ColumnCategory::Deploying => ColumnKind::Active,
        }
    }
}
text_enum!(Role {
    Lead => "lead", Coach => "coach", Devops => "devops", ReviewerArch => "reviewer.arch",
    ReviewerUx => "reviewer.ux", Tester => "tester", Dev => "dev",
    ReviewerCe => "reviewer.ce",
});
text_enum!(ItemType { Epic => "epic", Feature => "feature", Bug => "bug", Spike => "spike", Chore => "chore" });
text_enum!(Size { S => "S", M => "M", L => "L" });
text_enum!(Priority { P0 => "P0", P1 => "P1", P2 => "P2", P3 => "P3" });
text_enum!(PersonRole { Reviewer => "reviewer", Verifier => "verifier" });
text_enum!(VerificationResult { Pass => "pass", Fail => "fail", Blocked => "blocked" });
text_enum!(LinkKind {
    Task => "task", Decision => "decision", Artifact => "artifact", Branch => "branch", Pr => "pr",
    Meeting => "meeting", ItemBlocks => "item:blocks", ItemRelates => "item:relates",
    ItemDuplicates => "item:duplicates", Thread => "thread",
});
text_enum!(
    /// Where an item stands (0.19.0 K4b): a draft is on no column yet.
    ItemState { Draft => "draft", Live => "live", Discarded => "discarded" }
);
text_enum!(ItemEventKind {
    Created => "created", Moved => "moved", Edited => "edited", Commented => "commented",
    Linked => "linked", Assigned => "assigned", Blocked => "blocked", Ranked => "ranked",
    TaskDone => "task_done", TaskExpired => "task_expired", TaskCancelled => "task_cancelled",
    Unlinked => "unlinked",
});
text_enum!(TemplateKind { ItemType => "item_type", MeetingType => "meeting_type" });

#[derive(Debug, Clone, PartialEq)]
pub struct BoardSettings {
    pub project_id: String,
    pub key: String,
    pub next_seq: u32,
    pub stale_after_hours: u32,
    /// The machines that must verify an item carrying the label, by label.
    pub required_machines: BTreeMap<String, Vec<String>>,
    pub home_daemon_id: String,
    pub version: u64,
}

/// One board of a project: its own name, item-id prefix and structure policy.
#[derive(Debug, Clone, PartialEq)]
pub struct Board {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub key: String,
    pub description: String,
    pub ord: u32,
    pub is_default: bool,
    pub structure_policy: StructurePolicy,
    pub flow: BoardFlow,
    /// Fact name (`pr.merged`) → the column key a custom board moves to.
    pub automations: BTreeMap<String, String>,
    /// The label groups it declares, for filters and pickers.
    pub label_groups: Vec<super::labels::LabelGroup>,
    pub archived_at: Option<DateTime<Utc>>,
    pub version: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BoardColumn {
    pub project_id: String,
    /// Empty only in a mirror of a home older than 0.19.
    pub board_id: String,
    pub key: String,
    pub name: String,
    pub ord: u32,
    pub category: ColumnCategory,
    pub kind: ColumnKind,
    pub wip_limit: Option<u32>,
    pub wip_scope: WipScope,
    pub visible: bool,
    /// A custom board's rules for entering the column; unused on a
    /// release-gated board.
    pub policy: super::flow::ColumnPolicy,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProjectRole {
    pub project_id: String,
    pub role: Role,
    pub bot_id: String,
    pub machine: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Blocked {
    pub by: Option<String>,
    pub reason: String,
    pub since: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AcceptanceCriterion {
    pub idx: u32,
    pub text: String,
    pub checked: bool,
    pub checked_by: Option<String>,
    pub checked_at: Option<DateTime<Utc>>,
    pub machine: Option<String>,
    /// Provable only once installed: open at submit, required for Done.
    pub post_install: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ItemPerson {
    pub bot_id: String,
    pub role: PersonRole,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ItemVerification {
    pub machine: String,
    pub result: VerificationResult,
    pub by: String,
    pub at: DateTime<Utc>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub id: String,
    /// Empty only in a mirror of a home older than 0.19.
    pub board_id: String,
    pub seq: u32,
    pub item_type: ItemType,
    pub title: String,
    pub description: String,
    pub size: Option<Size>,
    pub priority: Priority,
    pub rank: String,
    pub column_key: String,
    pub category: ColumnCategory,
    pub blocked: Option<Blocked>,
    pub assignee: Option<String>,
    pub parent_id: Option<String>,
    pub release_id: Option<String>,
    pub labels: Vec<String>,
    pub acceptance_criteria: Vec<AcceptanceCriterion>,
    pub people: Vec<ItemPerson>,
    pub verifications: Vec<ItemVerification>,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub state_entered_at: DateTime<Utc>,
    pub done_at: Option<DateTime<Utc>>,
    pub version: u64,
    pub state: ItemState,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ItemCard {
    pub id: String,
    /// Empty only in a mirror of a home older than 0.19.
    pub board_id: String,
    pub item_type: ItemType,
    pub title: String,
    pub priority: Priority,
    pub size: Option<Size>,
    pub rank: String,
    pub column_key: String,
    pub assignee: Option<String>,
    pub labels: Vec<String>,
    pub blocked: bool,
    pub stale: bool,
    pub ac_checked: u32,
    pub ac_total: u32,
    pub version: u64,
    /// The owner's latest comment on it.
    pub owner_commented_at: Option<DateTime<Utc>>,
    /// A draft's card, for the owner's drafts tray.
    pub draft: bool,
    /// A draft's invited bots, by id; empty on a live card.
    pub invited: Vec<String>,
    /// Why it can't move on, filled when the card is read (`kanban::stuck`).
    pub move_block: Option<MoveBlock>,
}

/// What stops a card: its last refused move, or the next column's rules.
#[derive(Debug, Clone, PartialEq)]
pub struct MoveBlock {
    pub to_column: String,
    pub reason: String,
    pub more: u32,
    pub refused: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ItemLink {
    pub item_id: String,
    pub kind: LinkKind,
    pub target: String,
    pub label: Option<String>,
    pub created_by: String,
    pub at: DateTime<Utc>,
}

text_enum!(AttachmentKind { Image => "image", File => "file", Doc => "doc" });

/// A screenshot, a file or a linked document on an item (0.19.0 K4b).
#[derive(Debug, Clone, PartialEq)]
pub struct ItemAttachment {
    pub id: String,
    pub item_id: String,
    pub kind: AttachmentKind,
    pub name: String,
    pub mime: String,
    pub bytes: u64,
    pub sha256: String,
    /// Relative to the project's artifacts folder; empty for a URL.
    pub path: String,
    pub url: String,
    pub caption: String,
    pub added_by: String,
    pub at: DateTime<Utc>,
    pub removed: bool,
}

/// One change to a draft, with its author.
#[derive(Debug, Clone, PartialEq)]
pub struct DraftRevision {
    pub id: i64,
    pub item_id: String,
    pub actor: String,
    pub at: DateTime<Utc>,
    pub field: String,
    pub from: Option<String>,
    pub to: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ItemComment {
    pub id: String,
    pub item_id: String,
    pub author: String,
    pub body: String,
    pub reply_to: Option<String>,
    pub at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ItemEvent {
    pub id: i64,
    pub item_id: String,
    pub at: DateTime<Utc>,
    pub actor: String,
    pub kind: ItemEventKind,
    pub from: Option<String>,
    pub to: Option<String>,
    pub field: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Template {
    pub project_id: String,
    pub kind: TemplateKind,
    pub name: String,
    pub version: u32,
    pub body: serde_json::Value,
}

/// A guard condition a move does not meet yet, with what would fix it. The
/// UI shows these texts as they are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unmet {
    pub code: String,
    pub text: String,
    pub fix: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stored_spellings_parse_back() {
        for kind in LinkKind::ALL {
            assert_eq!(LinkKind::parse(kind.as_str()), Some(*kind));
        }
        assert_eq!(Role::parse("reviewer.ux"), Some(Role::ReviewerUx));
        assert_eq!(Size::parse("XL"), None);
    }
}
