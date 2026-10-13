//! Attachments and drafts (0.19.0 K4b).
//!
//! - `item.state`: `draft` (on no column yet: the owner and the bots invited
//!   to discuss it see it), `live` (every item before 0.19), or `discarded`
//!   (a draft the owner dropped, kept with its history). A draft keeps the
//!   board's first open column in `column_key`, which the foreign key needs,
//!   but no read of the board's columns counts it.
//! - `item_attachment`: a screenshot, a file or a linked document. The bytes
//!   live in the project's artifacts under `items/<item-id>/`; this is the
//!   record. A detached one keeps its row (`removed_at`), so a reference to
//!   it in the description shows a placeholder.
//! - `item_draft_invite`: the bots invited to discuss a draft.
//! - `item_draft_revision`: every change to a draft, with its author;
//!   `item_draft_seen` is the last one the owner has looked at.
//! - `item_link` gains the `thread` kind: the chat a draft was discussed in.
//!   SQLite can't alter a CHECK, so the table is rebuilt, ids kept.
//!
//! Safe to run again (see `db::migrate_columns`): the rebuild starts by
//! dropping a leftover copy, and copies every row whatever its kind.

pub(super) const MIGRATION_ITEM_DRAFTS: &str = r#"
ALTER TABLE item ADD COLUMN state TEXT NOT NULL DEFAULT 'live'
    CHECK(state IN ('draft', 'live', 'discarded'));
CREATE INDEX IF NOT EXISTS item_state ON item(project_id, state);

CREATE TABLE IF NOT EXISTS item_attachment (
    id         TEXT PRIMARY KEY,
    item_id    TEXT NOT NULL REFERENCES item(id),
    kind       TEXT NOT NULL CHECK(kind IN ('image', 'file', 'doc')),
    name       TEXT NOT NULL,
    mime       TEXT NOT NULL DEFAULT '',
    bytes      INTEGER NOT NULL DEFAULT 0,
    sha256     TEXT NOT NULL DEFAULT '',
    -- Relative to the project's artifacts folder; empty for a URL.
    path       TEXT NOT NULL DEFAULT '',
    url        TEXT NOT NULL DEFAULT '',
    caption    TEXT NOT NULL DEFAULT '',
    added_by   TEXT NOT NULL,
    at         TEXT NOT NULL,
    removed_at TEXT,
    removed_by TEXT
);
CREATE INDEX IF NOT EXISTS item_attachment_item ON item_attachment(item_id, at);

CREATE TABLE IF NOT EXISTS item_draft_invite (
    item_id    TEXT NOT NULL REFERENCES item(id),
    bot_id     TEXT NOT NULL,
    invited_by TEXT NOT NULL,
    at         TEXT NOT NULL,
    PRIMARY KEY (item_id, bot_id)
);

CREATE TABLE IF NOT EXISTS item_draft_revision (
    id      INTEGER PRIMARY KEY AUTOINCREMENT,
    item_id TEXT NOT NULL REFERENCES item(id),
    actor   TEXT NOT NULL,
    at      TEXT NOT NULL,
    field   TEXT NOT NULL,
    "from"  TEXT,
    "to"    TEXT
);
CREATE INDEX IF NOT EXISTS item_draft_revision_item ON item_draft_revision(item_id, id);

CREATE TABLE IF NOT EXISTS item_draft_seen (
    item_id TEXT PRIMARY KEY REFERENCES item(id),
    rev     INTEGER NOT NULL DEFAULT 0
);

DROP TABLE IF EXISTS item_link_next;
CREATE TABLE item_link_next (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    item_id    TEXT NOT NULL REFERENCES item(id),
    kind       TEXT NOT NULL CHECK(kind IN ('task', 'decision', 'artifact', 'branch', 'pr',
        'meeting', 'item:blocks', 'item:relates', 'item:duplicates', 'thread')),
    ref        TEXT NOT NULL,
    label      TEXT,
    created_by TEXT NOT NULL,
    at         TEXT NOT NULL,
    UNIQUE (item_id, kind, ref)
);
INSERT INTO item_link_next SELECT * FROM item_link;
DROP TABLE item_link;
ALTER TABLE item_link_next RENAME TO item_link;
"#;
