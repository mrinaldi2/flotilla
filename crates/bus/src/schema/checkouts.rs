//! Lighter checkouts (0.23.0, `repo_mirror_v1`): a project's disk budget and
//! the worktrees made per card from the project's mirror. Named rather than
//! numbered (ARCH-R1).

/// `project.disk_budget_bytes`: what the project's worktrees and build
/// caches may use on a computer; NULL, no budget. `card_worktree`: one row
/// per card worktree this computer made (`worktree_for`), with the folders
/// it is sparse to (a JSON array, empty for a full checkout) and the bot
/// that asked first.
pub(super) const MIGRATION_CARD_WORKTREES: &str = r#"
ALTER TABLE project ADD COLUMN disk_budget_bytes INTEGER;
CREATE TABLE IF NOT EXISTS card_worktree (
    project_id TEXT NOT NULL,
    item_id    TEXT NOT NULL,
    path       TEXT NOT NULL,
    branch     TEXT NOT NULL,
    base       TEXT NOT NULL,
    paths      TEXT NOT NULL DEFAULT '[]',
    bot_id     TEXT NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY (project_id, item_id)
);
"#;
