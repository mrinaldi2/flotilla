//! Board flows and column rules (0.19.0 K3).
//!
//! A board's `flow` is `release_gated` (the H-017 flow every board had
//! before 0.19, kept as one built-in flow) or `custom` (the columns' own
//! movers, entry rules and allowed sources decide). `automations` maps a
//! fact another module reports (`pr.merged`, `release.approved`) to the
//! column a custom board moves the item to; a release-gated board maps facts
//! the way 0.18 moved items.
//!
//! On a column, `movers` (JSON; NULL = any member), `rules` (JSON list) and
//! `allowed_from` (JSON list of column keys; empty = any) apply to custom
//! boards only.
//!
//! `item_fact` keeps the facts a board had no column for, so the card can
//! say "PR merged — no column mapped" (owner decision 3).
//!
//! Safe to run again (see `db::migrate_columns`).

pub(super) const MIGRATION_BOARD_FLOW: &str = r#"
ALTER TABLE board ADD COLUMN flow TEXT NOT NULL DEFAULT 'release_gated'
    CHECK(flow IN ('release_gated', 'custom'));
ALTER TABLE board ADD COLUMN automations TEXT NOT NULL DEFAULT '{}';

ALTER TABLE board_column ADD COLUMN movers TEXT;
ALTER TABLE board_column ADD COLUMN rules TEXT NOT NULL DEFAULT '[]';
ALTER TABLE board_column ADD COLUMN allowed_from TEXT NOT NULL DEFAULT '[]';

CREATE TABLE IF NOT EXISTS item_fact (
    id       INTEGER PRIMARY KEY,
    item_id  TEXT NOT NULL REFERENCES item(id),
    fact     TEXT NOT NULL,
    note     TEXT,
    actor    TEXT NOT NULL,
    at       TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS item_fact_item ON item_fact(item_id, at);
"#;
