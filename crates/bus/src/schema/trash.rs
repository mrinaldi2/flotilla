//! Deleting a project moves what it left on disk to the trash (0.19.0).
//! Named rather than numbered (ARCH-R1); safe to run again.

/// What a project delete asked of each linked computer that held the
/// project's other half, kept until that computer answers: written before
/// the link is dropped, sent at once, and again when the link comes back,
/// so a computer offline at the time still hears it.
///
/// `state` is `pending` until the computer answers (`confirmed`), or until
/// it has been asked `attempts` times on a live link without an answer
/// (`unconfirmed`): a computer from before 0.19 ignores the frame, and the
/// owner is told it didn't confirm. `outcome` is the computer's answer as
/// it sent it.
pub(super) const MIGRATION_PROJECT_TRASH: &str = r#"
CREATE TABLE IF NOT EXISTS project_trash_order (
    id                TEXT PRIMARY KEY,
    project_id        TEXT NOT NULL,
    project_name      TEXT NOT NULL,
    peer_id           TEXT NOT NULL,
    remote_project_id TEXT NOT NULL,
    state             TEXT NOT NULL
        CHECK(state IN ('pending', 'confirmed', 'unconfirmed')),
    attempts          INTEGER NOT NULL DEFAULT 0,
    sent_at           TEXT,
    outcome           TEXT,
    moved_bytes       INTEGER NOT NULL DEFAULT 0,
    created_at        TEXT NOT NULL,
    updated_at        TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_project_trash_order_pending
    ON project_trash_order(peer_id) WHERE state = 'pending';
"#;
