//! The plan (0.21.0, #43, plan.proto): a project's goals, initiatives, epics
//! and features. Board cards point at a node through `item.plan_node_id`
//! (migration 072); a node is never a card.
//!
//! - `plan_settings`: planning on or off per project, and the node ids'
//!   sequence (`PL-1`, `PL-2`, …), one per project.
//! - `plan_node`: a node, its stage and its research. An initiative names
//!   the board its subtree's cards go on.
//! - `plan_doc`: a node's documents in the project's repository, with the
//!   commit the owner approved each at.
//! - `plan_gate`: what waits for the owner's ruling on a node, at most one.
//! - `plan_event`: a node's history.

pub(super) const MIGRATION_PLAN: &str = r#"
CREATE TABLE IF NOT EXISTS plan_settings (
    project_id TEXT PRIMARY KEY REFERENCES project(id),
    enabled    INTEGER NOT NULL DEFAULT 0,
    next_seq   INTEGER NOT NULL DEFAULT 1,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS plan_node (
    id                TEXT PRIMARY KEY,
    project_id        TEXT NOT NULL REFERENCES project(id),
    kind              TEXT NOT NULL CHECK(kind IN ('goal', 'initiative', 'epic', 'feature')),
    parent_id         TEXT REFERENCES plan_node(id),
    title             TEXT NOT NULL,
    summary           TEXT NOT NULL DEFAULT '',
    stage             TEXT NOT NULL CHECK(stage IN ('define', 'active', 'met', 'dropped',
        'proposed', 'research', 'prd', 'design', 'hld', 'lld', 'running', 'build', 'released',
        'done')),
    due               TEXT,
    lead              TEXT,
    board_id          TEXT REFERENCES board(id),
    research          TEXT CHECK(research IN ('open', 'closed', 'skipped')),
    research_closed_at TEXT,
    research_reason   TEXT,
    rank              TEXT NOT NULL,
    created_by        TEXT NOT NULL,
    created_at        TEXT NOT NULL,
    updated_at        TEXT NOT NULL,
    version           INTEGER NOT NULL DEFAULT 1
);
CREATE INDEX IF NOT EXISTS plan_node_project ON plan_node(project_id, parent_id, rank);

CREATE TABLE IF NOT EXISTS plan_doc (
    node_id     TEXT NOT NULL REFERENCES plan_node(id),
    kind        TEXT NOT NULL CHECK(kind IN ('goal', 'research', 'prd', 'design', 'hld', 'lld')),
    path        TEXT NOT NULL,
    item_id     TEXT,
    approved_sha TEXT,
    approved_at TEXT,
    PRIMARY KEY (node_id, kind, path)
);

CREATE TABLE IF NOT EXISTS plan_gate (
    node_id      TEXT PRIMARY KEY REFERENCES plan_node(id),
    stage        TEXT NOT NULL,
    doc_path     TEXT,
    sha          TEXT,
    pr_number    INTEGER,
    submitted_by TEXT NOT NULL,
    submitted_at TEXT NOT NULL,
    note         TEXT,
    children     TEXT NOT NULL DEFAULT '[]',
    round        INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE IF NOT EXISTS plan_event (
    id      INTEGER PRIMARY KEY AUTOINCREMENT,
    node_id TEXT NOT NULL REFERENCES plan_node(id),
    at      TEXT NOT NULL,
    actor   TEXT NOT NULL,
    kind    TEXT NOT NULL,
    stage   TEXT,
    note    TEXT
);
CREATE INDEX IF NOT EXISTS plan_event_node ON plan_event(node_id, id);
"#;
