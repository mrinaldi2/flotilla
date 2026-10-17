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

    /// Every string above, in the order listed.
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
