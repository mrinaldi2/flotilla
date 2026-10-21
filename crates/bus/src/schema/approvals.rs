//! Commands and approvals without the chat (0.24.0): the owner's Stop on a
//! bot's command, push requests as owner actions, and the research gate.
//! Named rather than numbered (ARCH-R1).

/// `owner_action.kind` (`push_requests_v1`): `command`, what every action
/// was before, or `push`, a bot's `request_push`, whose `push` holds the
/// details read from its worktree (JSON). Both are written once: the
/// immutability trigger is made again to cover them. `command_stop`
/// (`commands_v1`): a bot's command the owner stopped, by the bot's id and
/// the command's tool call id.
pub(super) const MIGRATION_PUSH_REQUESTS: &str = r#"
ALTER TABLE owner_action ADD COLUMN kind TEXT NOT NULL DEFAULT 'command';
ALTER TABLE owner_action ADD COLUMN push TEXT;
DROP TRIGGER IF EXISTS owner_action_immutable;
CREATE TRIGGER owner_action_immutable
BEFORE UPDATE OF project_id, proposed_by, item_id, decision_id, target_machine, shell, cwd,
    content, pinned_files, reason, timeout_s, sha256, origin, created_at, local_project_id,
    kind, push
    ON owner_action
BEGIN
    SELECT RAISE(ABORT, 'an owner action is immutable once proposed');
END;

CREATE TABLE IF NOT EXISTS command_stop (
    bot_id     TEXT NOT NULL,
    command_id TEXT NOT NULL,
    command    TEXT NOT NULL,
    stopped_by TEXT NOT NULL,
    stopped_at TEXT NOT NULL,
    PRIMARY KEY (bot_id, command_id)
);
"#;

/// The research gate (`research_gate_v1`): `plan_settings.research_gate`,
/// on by default, and `awaiting` among a node's research states, which
/// needs `plan_node` rebuilt for its CHECK list (with foreign keys off; its
/// rows, indexes and the tables naming it kept).
pub(super) const MIGRATION_RESEARCH_GATE: &str = r#"-- foreign_keys: off
ALTER TABLE plan_settings ADD COLUMN research_gate INTEGER NOT NULL DEFAULT 1;
DROP TABLE IF EXISTS plan_node_next;
CREATE TABLE plan_node_next (
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
    research          TEXT CHECK(research IN ('open', 'closed', 'skipped', 'awaiting')),
    research_closed_at TEXT,
    research_reason   TEXT,
    rank              TEXT NOT NULL,
    created_by        TEXT NOT NULL,
    created_at        TEXT NOT NULL,
    updated_at        TEXT NOT NULL,
    version           INTEGER NOT NULL DEFAULT 1
);
INSERT INTO plan_node_next(id, project_id, kind, parent_id, title, summary, stage, due, lead,
    board_id, research, research_closed_at, research_reason, rank, created_by, created_at,
    updated_at, version)
    SELECT id, project_id, kind, parent_id, title, summary, stage, due, lead, board_id,
        research, research_closed_at, research_reason, rank, created_by, created_at,
        updated_at, version
    FROM plan_node;
DROP TABLE plan_node;
ALTER TABLE plan_node_next RENAME TO plan_node;
CREATE INDEX IF NOT EXISTS plan_node_project ON plan_node(project_id, parent_id, rank);
"#;
