//! The board domain: its own types (`model`), the mapping to
//! the wire contract (`contract`), defaults and ranking here; storage in
//! `db::board*`. The move guards (`guards`) and the move engine that WS, MCP
//! and the peer link call (`moves`), and the pushes they publish (`feed`).
//! The repo's review and check policy (`policy`). Release packages and the
//! deploy gate live in `crate::releases`.

pub mod admin;
pub mod api;
pub mod attach;
pub mod attachments;
pub mod contract;
pub mod defaults;
pub mod draft_talk;
pub mod draft_undo;
pub mod drafts;
pub mod edit;
pub mod facts;
pub mod feed;
pub mod flow;
pub mod guards;
pub mod handback;
pub mod keeper;
pub mod kickoffs;
pub mod labels;
pub mod metrics;
pub mod mirror;
pub mod model;
pub mod moves;
pub mod notice_text;
pub mod notices;
pub mod presets;
pub mod rank;
pub mod starts;
pub mod stuck;
pub mod team;
