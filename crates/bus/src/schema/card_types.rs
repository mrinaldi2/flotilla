//! 0.21 card types and the Research column (#43, step 2).
//!
//! - Features and epics leave the board for the plan: every epic and
//!   feature card is deleted with everything that hangs off it (the owner's
//!   decision; the daemon backs the database up before migrating). Each one
//!   is first recorded in `item_purged`, which the daemon reads afterwards to
//!   remove the cards' attachment folders and tell their assignees.
//!   Their child cards stay and lose the parent.
//! - Chores become tasks. The item-type templates follow in the daemon
//!   (`db::card_types`): a task takes a feature's Definition of Ready.
//! - `item.type` takes `task | design | bug | spike`, and items gain
//!   `plan_node_id`; `board_column.category` gains `research`. SQLite can't
//!   alter a CHECK and both tables are referenced, so both are rebuilt with
//!   foreign keys off (the first line tells the runner), names and rows kept.
//!
//! Safe to run again: the rebuilds start by dropping a leftover copy, and the
//! deletes find nothing the second time.

pub(super) const MIGRATION_CARD_TYPES: &str = r#"-- foreign_keys: off
CREATE TABLE IF NOT EXISTS item_purged (
    id          TEXT PRIMARY KEY,
    project_id  TEXT NOT NULL,
    type        TEXT NOT NULL,
    title       TEXT NOT NULL,
    assignee    TEXT,
    column_key  TEXT NOT NULL,
    purged_at   TEXT NOT NULL,
    -- Set once the daemon removed its files and told its assignee.
    handled_at  TEXT
);
INSERT OR IGNORE INTO item_purged(id, project_id, type, title, assignee, column_key, purged_at)
    SELECT id, project_id, type, title, assignee, column_key, strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
    FROM item WHERE type IN ('epic', 'feature');

CREATE TEMP TABLE gone AS SELECT id FROM item WHERE type IN ('epic', 'feature');

DELETE FROM item_ac WHERE item_id IN (SELECT id FROM gone);
DELETE FROM item_ac_post_install WHERE item_id IN (SELECT id FROM gone);
DELETE FROM item_person WHERE item_id IN (SELECT id FROM gone);
DELETE FROM item_verification WHERE item_id IN (SELECT id FROM gone);
DELETE FROM item_link WHERE item_id IN (SELECT id FROM gone);
DELETE FROM item_comment WHERE item_id IN (SELECT id FROM gone);
DELETE FROM item_event WHERE item_id IN (SELECT id FROM gone);
DELETE FROM item_fact WHERE item_id IN (SELECT id FROM gone);
DELETE FROM item_attachment WHERE item_id IN (SELECT id FROM gone);
DELETE FROM item_draft_invite WHERE item_id IN (SELECT id FROM gone);
DELETE FROM item_draft_revision WHERE item_id IN (SELECT id FROM gone);
DELETE FROM item_draft_seen WHERE item_id IN (SELECT id FROM gone);
DELETE FROM worker_item WHERE item_id IN (SELECT id FROM gone);
DELETE FROM release_item WHERE item_id IN (SELECT id FROM gone);
DELETE FROM release_work_item WHERE item_id IN (SELECT id FROM gone);
DELETE FROM keeper_mark WHERE item_id IN (SELECT id FROM gone);
DELETE FROM card_question WHERE item_id IN (SELECT id FROM gone);
UPDATE task SET item_id = NULL WHERE item_id IN (SELECT id FROM gone);
UPDATE decision SET item_id = NULL WHERE item_id IN (SELECT id FROM gone);
UPDATE action_item SET item_id = NULL WHERE item_id IN (SELECT id FROM gone);
UPDATE owner_question SET item_id = NULL WHERE item_id IN (SELECT id FROM gone);
UPDATE item SET parent_id = NULL WHERE parent_id IN (SELECT id FROM gone);
-- Links from other cards to a removed one.
DELETE FROM item_link
    WHERE kind IN ('item:blocks', 'item:relates', 'item:duplicates')
      AND ref IN (SELECT id FROM gone);
DELETE FROM item WHERE id IN (SELECT id FROM gone);
DROP TABLE gone;

-- `design`: the Design card a task builds.
DROP TABLE IF EXISTS item_link_next;
CREATE TABLE item_link_next (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    item_id    TEXT NOT NULL REFERENCES item(id),
    kind       TEXT NOT NULL CHECK(kind IN ('task', 'decision', 'artifact', 'branch', 'pr',
        'meeting', 'item:blocks', 'item:relates', 'item:duplicates', 'thread', 'design')),
    ref        TEXT NOT NULL,
    label      TEXT,
    created_by TEXT NOT NULL,
    at         TEXT NOT NULL,
    UNIQUE (item_id, kind, ref)
);
INSERT INTO item_link_next SELECT * FROM item_link;
DROP TABLE item_link;
ALTER TABLE item_link_next RENAME TO item_link;

DROP TABLE IF EXISTS board_column_next;
CREATE TABLE board_column_next (
    project_id   TEXT NOT NULL REFERENCES project(id),
    key          TEXT NOT NULL,
    name         TEXT NOT NULL,
    ord          INTEGER NOT NULL,
    category     TEXT NOT NULL CHECK(category IN
        ('inbox', 'ready', 'research', 'doing', 'review', 'verify', 'approval', 'deploying',
         'done', 'cancelled')),
    wip_limit    INTEGER,
    wip_scope    TEXT NOT NULL DEFAULT 'column' CHECK(wip_scope IN ('column', 'per_assignee')),
    visible      INTEGER NOT NULL DEFAULT 1,
    movers       TEXT,
    rules        TEXT NOT NULL DEFAULT '[]',
    allowed_from TEXT NOT NULL DEFAULT '[]',
    board_id     TEXT REFERENCES board(id),
    kind         TEXT CHECK(kind IN ('open', 'active', 'done', 'cancelled')),
    PRIMARY KEY (project_id, key)
);
INSERT INTO board_column_next(project_id, key, name, ord, category, wip_limit, wip_scope,
        visible, movers, rules, allowed_from, board_id, kind)
    SELECT project_id, key, name, ord, category, wip_limit, wip_scope,
        visible, movers, rules, allowed_from, board_id, kind
    FROM board_column;
DROP TABLE board_column;
ALTER TABLE board_column_next RENAME TO board_column;
CREATE INDEX IF NOT EXISTS board_column_board ON board_column(board_id, ord);

DROP TABLE IF EXISTS item_next;
CREATE TABLE item_next (
    id               TEXT PRIMARY KEY,
    project_id       TEXT NOT NULL REFERENCES project(id),
    seq              INTEGER NOT NULL,
    type             TEXT NOT NULL CHECK(type IN ('task', 'design', 'bug', 'spike')),
    title            TEXT NOT NULL,
    description      TEXT NOT NULL DEFAULT '',
    platforms        TEXT NOT NULL DEFAULT '[]',
    size             TEXT CHECK(size IN ('S', 'M', 'L')),
    priority         TEXT NOT NULL DEFAULT 'P2' CHECK(priority IN ('P0', 'P1', 'P2', 'P3')),
    rank             TEXT NOT NULL,
    column_key       TEXT NOT NULL,
    category         TEXT NOT NULL,
    blocked_by       TEXT,
    blocked_reason   TEXT,
    blocked_since    TEXT,
    assignee         TEXT,
    parent_id        TEXT REFERENCES item(id),
    release_id       TEXT,
    labels           TEXT NOT NULL DEFAULT '[]',
    created_by       TEXT NOT NULL,
    created_at       TEXT NOT NULL,
    updated_at       TEXT NOT NULL,
    state_entered_at TEXT NOT NULL,
    done_at          TEXT,
    version          INTEGER NOT NULL DEFAULT 1,
    state            TEXT NOT NULL DEFAULT 'live' CHECK(state IN ('draft', 'live', 'discarded')),
    board_id         TEXT REFERENCES board(id),
    -- The plan node it belongs to (plan.proto); no foreign key, because the
    -- plan's tables come later and a peer's node may not be here.
    plan_node_id     TEXT,
    UNIQUE (project_id, seq),
    FOREIGN KEY (project_id, column_key) REFERENCES board_column(project_id, key)
);
INSERT INTO item_next(id, project_id, seq, type, title, description, platforms, size, priority,
        rank, column_key, category, blocked_by, blocked_reason, blocked_since, assignee,
        parent_id, release_id, labels, created_by, created_at, updated_at, state_entered_at,
        done_at, version, state, board_id)
    SELECT id, project_id, seq, CASE type WHEN 'chore' THEN 'task' ELSE type END, title,
        description, platforms, size, priority,
        rank, column_key, category, blocked_by, blocked_reason, blocked_since, assignee,
        parent_id, release_id, labels, created_by, created_at, updated_at, state_entered_at,
        done_at, version, state, board_id
    FROM item;
DROP TABLE item;
ALTER TABLE item_next RENAME TO item;
CREATE INDEX IF NOT EXISTS idx_item_column ON item(project_id, column_key, rank);
CREATE INDEX IF NOT EXISTS idx_item_assignee ON item(project_id, assignee);
CREATE INDEX IF NOT EXISTS idx_item_parent ON item(parent_id);
CREATE INDEX IF NOT EXISTS item_state ON item(project_id, state);
CREATE INDEX IF NOT EXISTS item_board ON item(board_id);
CREATE INDEX IF NOT EXISTS item_plan_node ON item(plan_node_id);

CREATE TRIGGER IF NOT EXISTS item_fts_ai AFTER INSERT ON item BEGIN
    INSERT INTO item_fts(item_id, title, description, comments)
    VALUES (new.id, new.title, new.description, '');
END;
CREATE TRIGGER IF NOT EXISTS item_fts_au AFTER UPDATE OF title, description ON item BEGIN
    UPDATE item_fts SET title = new.title, description = new.description
    WHERE item_id = new.id;
END;
CREATE TRIGGER IF NOT EXISTS item_fts_ad AFTER DELETE ON item BEGIN
    DELETE FROM item_fts WHERE item_id = old.id;
END;
DELETE FROM item_fts WHERE item_id IN (SELECT id FROM item_purged);
"#;
