//! Many boards per project (0.19.0 K2). A board has its own name, item-id
//! prefix and structure policy; columns, items and templates belong to one.
//! Every project with a board gets it back as its default board, "Main",
//! keeping its key, so no item id changes.
//!
//! Item numbers stay one sequence per project (`board_settings.next_seq`),
//! which keeps `item(project_id, seq)` unique without rebuilding `item`;
//! a board's key is the prefix of the items created on it. Column keys stay
//! unique per project for the same reason (`board_column`'s key and the
//! `item(project_id, column_key)` foreign key are unchanged).
//!
//! `kind` is the meaning Kanban needs from a column: `open` (not started),
//! `active` (WIP, staleness and cycle time apply), `done` and `cancelled`.
//! `category` stays until the column rules replace it (K3).
//!
//! Safe to run again: the daemon skips an `ADD COLUMN` whose column exists
//! (`db::migrate_columns`), and the rest only fills what is still unset.

pub(super) const MIGRATION_BOARDS: &str = r#"
CREATE TABLE IF NOT EXISTS board (
    id               TEXT PRIMARY KEY,
    project_id       TEXT NOT NULL REFERENCES project(id),
    name             TEXT NOT NULL,
    -- Item id prefix, unique across the daemon so item ids are too.
    key              TEXT NOT NULL UNIQUE,
    description      TEXT NOT NULL DEFAULT '',
    ord              INTEGER NOT NULL DEFAULT 0,
    is_default       INTEGER NOT NULL DEFAULT 0,
    structure_policy TEXT NOT NULL DEFAULT 'owner_confirms'
        CHECK(structure_policy IN ('owner_confirms', 'lead_free')),
    archived_at      TEXT,
    version          INTEGER NOT NULL DEFAULT 1,
    created_at       TEXT NOT NULL,
    updated_at       TEXT NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS board_one_default ON board(project_id) WHERE is_default = 1;
CREATE INDEX IF NOT EXISTS board_project ON board(project_id, ord);

INSERT OR IGNORE INTO board(id, project_id, name, key, is_default, created_at, updated_at)
    SELECT project_id || '/main', project_id, 'Main', key, 1, created_at, updated_at
    FROM board_settings;

ALTER TABLE board_column ADD COLUMN board_id TEXT REFERENCES board(id);
ALTER TABLE board_column ADD COLUMN kind TEXT
    CHECK(kind IN ('open', 'active', 'done', 'cancelled'));
UPDATE board_column SET
    board_id = project_id || '/main',
    kind = CASE category
        WHEN 'inbox' THEN 'open'
        WHEN 'ready' THEN 'open'
        WHEN 'done' THEN 'done'
        WHEN 'cancelled' THEN 'cancelled'
        ELSE 'active'
    END
WHERE board_id IS NULL;
CREATE INDEX IF NOT EXISTS board_column_board ON board_column(board_id, ord);

ALTER TABLE item ADD COLUMN board_id TEXT REFERENCES board(id);
UPDATE item SET board_id = project_id || '/main' WHERE board_id IS NULL;
CREATE INDEX IF NOT EXISTS item_board ON item(board_id);

-- NULL: the project-wide template every board starts from.
ALTER TABLE template ADD COLUMN board_id TEXT REFERENCES board(id);
"#;
