//! A lead's proposed structure change (0.19.0 K4): under a board's
//! `owner_confirms` policy, a lead's change to columns, rules or automations
//! is kept here and raised as a decision; the owner's approval applies it,
//! a decline drops it. `request` is the structure request as proto JSON,
//! applied as sent against the board `version` it was proposed on.

pub(super) const MIGRATION_BOARD_PROPOSALS: &str = r#"
CREATE TABLE IF NOT EXISTS board_proposal (
    id          TEXT PRIMARY KEY,
    project_id  TEXT NOT NULL REFERENCES project(id),
    board_id    TEXT REFERENCES board(id),
    request     TEXT NOT NULL,
    summary     TEXT NOT NULL,
    proposed_by TEXT NOT NULL,
    decision_id TEXT REFERENCES decision(id),
    state       TEXT NOT NULL DEFAULT 'open'
        CHECK(state IN ('open', 'applied', 'declined', 'stale')),
    note        TEXT,
    created_at  TEXT NOT NULL,
    ruled_at    TEXT
);
CREATE INDEX IF NOT EXISTS board_proposal_open ON board_proposal(project_id, state);
"#;
