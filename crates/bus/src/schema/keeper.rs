//! The board keeper's memory (0.19.0): what the daemon's keeper already did
//! about a card, so it acts once per condition and a restart doesn't make it
//! act again. One row per card and rule:
//!
//! - `rule`: `silent_nudged`, `silent_stale`, `not_started`, `not_moved`,
//!   `blocked` or `stale`;
//! - `assignee` and `column_key`: the card as it was when the row was
//!   written; a row whose card has since moved or changed hands is no
//!   longer true (`stale` keeps only its column);
//! - `since`: when the condition began (the assignment a reminder is about,
//!   the moment a move was first seen refused);
//! - `fired_at`: when the keeper acted on it; unset while it only waits;
//! - `detail`: what it is about, when the rule needs to say (the refused
//!   column and rule).
//!
//! No foreign key: items are never deleted, and the keeper's rows must never
//! hold up anything else. Safe to run again.

pub(super) const MIGRATION_BOARD_KEEPER: &str = r#"
CREATE TABLE IF NOT EXISTS keeper_mark (
    item_id    TEXT NOT NULL,
    rule       TEXT NOT NULL,
    assignee   TEXT NOT NULL DEFAULT '',
    column_key TEXT NOT NULL,
    since      TEXT NOT NULL,
    fired_at   TEXT,
    detail     TEXT NOT NULL DEFAULT '',
    PRIMARY KEY (item_id, rule)
);
"#;
