//! The typed wire contract, generated from `proto/flotilla/<surface>/v1/*.proto`
//! (ADR-001 §1) by `build.rs`. Nothing here is written by hand except the
//! version map. Golden proto-JSON fixtures under `crates/bus/fixtures/` pin
//! the shapes for Rust, TypeScript and Swift.
//!
//! Each surface carries its own version, advertised in `hello_ok.contracts`
//! and sent by clients in `hello.contracts`. Within a version protobuf's own
//! rules carry additive change; when `buf breaking` fails, the surface's
//! version is bumped.

use std::collections::BTreeMap;

/// The well-known types the generated messages use (Timestamp, Struct), so
/// code mapping to and from them needs no dependency of its own.
pub use pbjson_types;

/// The generated code, nested as its proto packages are, so a message of one
/// package can name another's (the wire `Envelope` carries the board's).
#[allow(clippy::all, clippy::pedantic, missing_docs)]
mod flotilla {
    pub mod board {
        pub mod v1 {
            include!(concat!(env!("OUT_DIR"), "/flotilla.board.v1.rs"));
            include!(concat!(env!("OUT_DIR"), "/flotilla.board.v1.serde.rs"));
        }
    }
    pub mod home {
        pub mod v1 {
            include!(concat!(env!("OUT_DIR"), "/flotilla.home.v1.rs"));
            include!(concat!(env!("OUT_DIR"), "/flotilla.home.v1.serde.rs"));
        }
    }
    pub mod pr {
        pub mod v1 {
            include!(concat!(env!("OUT_DIR"), "/flotilla.pr.v1.rs"));
            include!(concat!(env!("OUT_DIR"), "/flotilla.pr.v1.serde.rs"));
        }
    }
    pub mod wire {
        pub mod v1 {
            include!(concat!(env!("OUT_DIR"), "/flotilla.wire.v1.rs"));
            include!(concat!(env!("OUT_DIR"), "/flotilla.wire.v1.serde.rs"));
        }
    }
}

/// The board surface (H-017): entities, requests, responses and pushes.
pub mod board {
    pub use super::flotilla::board::v1::*;

    /// Bumped only on a breaking change to this surface.
    pub const VERSION: u32 = 1;

    /// JSON Schemas of the board's messages, by name, generated from the
    /// protos at build time; the MCP board tools are built on them.
    pub const MESSAGE_SCHEMAS: &str =
        include_str!(concat!(env!("OUT_DIR"), "/board_messages.schema.json"));
}

/// The projects home (H-128): the overview, attention rows, and the peer
/// messages that build them across computers.
pub mod home {
    pub use super::flotilla::home::v1::*;

    /// Bumped only on a breaking change to this surface.
    pub const VERSION: u32 = 1;
}

/// Pull requests and checks (H-261): PRs bound to the commits reviewed,
/// checks per commit, cleanup after merge, and the bots' tools.
pub mod pr {
    pub use super::flotilla::pr::v1::*;

    /// Bumped only on a breaking change to this surface.
    pub const VERSION: u32 = 1;

    /// The `hello_ok.capabilities` this surface goes with, advertised since
    /// the daemon serves its reads and pushes (H-273).
    pub const CAPABILITIES: &[&str] = &["pull_requests", "checks"];
}

/// `hello_ok.capabilities` strings for the 0.19 and 0.20 board and home
/// additions, named once for the daemon and the clients' docs
/// (docs/protocol.md). A daemon advertises one only once it serves all of
/// it; a client sends the request or reads the field only when it is there.
pub mod capability {
    /// The 0.19 board structure (named after the fact, 0.20.0): several
    /// boards per project from presets (`board_list`, `board_create`,
    /// `board_update`), custom columns (`column_add`, `column_update`,
    /// `column_remove`, `automation_set`), the lead's proposals
    /// (`board_proposal_rule`), `role_set` and `project_delete_preview`.
    pub const BOARDS_0_19: &str = "boards_0_19";
    /// `BoardEnable.preset` and `BoardEnable.columns` (0.20.0).
    pub const BOARD_PRESETS: &str = "board_presets";
    /// `description` on `create_project`, `update_project` and projects.
    pub const PROJECT_DESCRIPTION: &str = "project_description";
    /// `create_bot {lead: true}`.
    pub const CREATE_BOT_LEAD: &str = "create_bot_lead";
    /// `ProjectRow.bot_list`, `ProjectAttention.Part.bots`, `Member.online`
    /// and `Member.last_seen_at`.
    pub const OVERVIEW_BOTS: &str = "overview_bots";
    /// `ProjectRow.top_attention`.
    pub const OVERVIEW_TOP_ATTENTION: &str = "overview_top_attention";
    /// `AttentionKind::BotStopped` rows, with `AttentionRow.bot_state`.
    pub const ATTENTION_BOT_STOPPED: &str = "attention_bot_stopped";
    /// `clone_project_repo`, its `repo_clone_started` answer and the
    /// `repo_clone` pushes.
    pub const PROJECT_REPO_CLONE: &str = "project_repo_clone";
    /// The plan (0.21.0, plan.proto): the `plan_*` board requests, the
    /// `planned`, `plan`, `plan_node` and `plan_doc` answers, the
    /// `plan_change` push, and `plan_node_id` on items, cards and queries.
    pub const PLAN_V1: &str = "plan_v1";
    /// 0.21 card types (0.21.0): `ItemType::Task` and `ItemType::Design`;
    /// creating or retyping to epic, feature or chore is refused.
    pub const ITEM_TYPES_V2: &str = "item_types_v2";
    /// `ColumnCategory::Research` columns and the `start.research` guard, and
    /// `LinkKind::Design` (0.21.0).
    pub const COLUMN_RESEARCH: &str = "column_research";
    /// `AttentionKind::PlanGate` rows targeting `plan_node_id` (0.21.0).
    pub const ATTENTION_PLAN: &str = "attention_plan";
    /// Questions for the owner (0.22.0, questions.proto): `ask_owner`,
    /// `question_list`, `question_answer` and `question_dismiss`, the
    /// `questions` and `questioned` answers, the `question_change` push,
    /// `questions` on item and plan node details, the open counts on cards
    /// and nodes, `AttentionKind::Question` rows, and `plan.question_open`.
    pub const QUESTIONS_V1: &str = "questions_v1";
    /// Artifacts by kind (0.22.0): `list_artifacts {group}`, each file's
    /// `group` and `used_by`, the storage folders left out, and a card's
    /// artifact links listed with its documents.
    pub const ARTIFACTS_V2: &str = "artifacts_v2";

    /// Every string above, in the order listed.
    /// The on-demand team (0.23.0): bot `presence`, the `asleep` and `queued`
    /// states, `set_bot_presence`, `wake_bot`, `sleep_bot`, `team_status`, the
    /// `team_updated` push, the `max_running_bots` and `idle_sleep_minutes`
    /// settings, `ROLE_PRODUCT`/`ROLE_ARCHITECT` and assigning by role.
    pub const TEAM_ON_DEMAND_V1: &str = "team_on_demand_v1";
    /// Lighter checkouts (0.23.0): the project's mirror, worktrees per card,
    /// the read-only repo tools and the project's disk budget.
    pub const REPO_MIRROR_V1: &str = "repo_mirror_v1";
    /// The board runs the SDLC (0.24.0): `BoardColumn.on_entry`, its
    /// `column_add`/`column_update` field, the presets' default chain, the
    /// Tester closing a passed card, and `AttentionKind::RoleUnheld` rows.
    pub const BOARD_FLOW_V1: &str = "board_flow_v1";
    /// Commands across bots (0.24.0): `list_commands`, the `commands`
    /// answer, the `commands_changed` push and the owner's `stop_command`.
    pub const COMMANDS_V1: &str = "commands_v1";
    /// Push requests (0.24.0): the bots' `request_push` tool, owner actions
    /// of `kind: "push"` with their `push` details, run from desktop or phone.
    pub const PUSH_REQUESTS_V1: &str = "push_requests_v1";
    /// The research gate (0.24.0): `PlanResearchState::Awaiting`, the
    /// RESEARCH gate on findings, and `research_gate` on `plan_enable` and
    /// `PlanTree`.
    pub const RESEARCH_GATE_V1: &str = "research_gate_v1";
    /// Usage and budgets (0.24.0): `usage_get`, `set_project_budget`,
    /// `budget_extend`, the price table, `usage_updated`, `ProjectRow.usage`,
    /// the budget attention rows, and `rate_limited` set from the runtime.
    pub const USAGE_V1: &str = "usage_v1";
    pub const ALL: &[&str] = &[
        BOARDS_0_19,
        BOARD_PRESETS,
        PROJECT_DESCRIPTION,
        CREATE_BOT_LEAD,
        OVERVIEW_BOTS,
        OVERVIEW_TOP_ATTENTION,
        ATTENTION_BOT_STOPPED,
        PROJECT_REPO_CLONE,
        PLAN_V1,
        ITEM_TYPES_V2,
        COLUMN_RESEARCH,
        ATTENTION_PLAN,
        QUESTIONS_V1,
        ARTIFACTS_V2,
        TEAM_ON_DEMAND_V1,
        REPO_MIRROR_V1,
        BOARD_FLOW_V1,
        COMMANDS_V1,
        PUSH_REQUESTS_V1,
        RESEARCH_GATE_V1,
        USAGE_V1,
    ];
}

/// What a WebSocket binary frame carries: an `Envelope` per frame.
pub mod wire {
    pub use super::flotilla::wire::v1::*;
}

/// Every contract surface this build speaks, with its version.
pub const CONTRACTS: &[(&str, u32)] = &[
    ("board", board::VERSION),
    ("home", home::VERSION),
    ("pr", pr::VERSION),
];

/// Binary encodings this build accepts, advertised in `hello_ok.encodings`.
pub const ENCODINGS: &[&str] = &["proto"];

/// [`CONTRACTS`] as the map `hello_ok` carries.
pub fn versions() -> BTreeMap<String, u32> {
    CONTRACTS
        .iter()
        .map(|(name, version)| ((*name).to_string(), *version))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_list_every_surface() {
        assert_eq!(versions().get("board"), Some(&board::VERSION));
        assert_eq!(versions().get("home"), Some(&home::VERSION));
        assert_eq!(versions().get("pr"), Some(&pr::VERSION));
    }
}
