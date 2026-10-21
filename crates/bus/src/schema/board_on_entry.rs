//! The board runs the SDLC (0.24.0, `board_flow_v1`).
//!
//! `board_column.on_entry` is the role a card entering the column is
//! assigned to (NULL: nobody). `role_wait` keeps the cards that entered such
//! a column while nobody held its role, for the `ROLE_UNHELD` row in Needs
//! you: one row per card, replaced when it moves again. No foreign key: a
//! card deleted with its project leaves a row nothing reads.
//!
//! Safe to run again (see `db::migrate_columns`).

pub(super) const MIGRATION_BOARD_ON_ENTRY: &str = r#"
ALTER TABLE board_column ADD COLUMN on_entry TEXT;

CREATE TABLE IF NOT EXISTS role_wait (
    item_id    TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    board_id   TEXT NOT NULL,
    column_key TEXT NOT NULL,
    role       TEXT NOT NULL,
    since      TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS role_wait_board ON role_wait(project_id, board_id, role);
"#;
