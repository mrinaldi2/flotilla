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

/// `project.pr_flow` (0.23.1): the owner turned the project's pull requests
/// on in the app, beside the daemon config's `pr_flow_projects`.
pub(super) const MIGRATION_PROJECT_PR_FLOW: &str = r#"
ALTER TABLE project ADD COLUMN pr_flow INTEGER NOT NULL DEFAULT 0;
"#;

/// 0.23.3: a PR on the project's own repository lands on that repository's
/// branch, but every PR was recorded as landing on `main`. Its label takes
/// the branch the owner set; PRs on added repositories keep theirs.
pub(super) const MIGRATION_PR_BASE_LABEL: &str = r#"
UPDATE pr SET base = (
    SELECT r.branch FROM project_repo r WHERE r.project_id = pr.project_id
)
WHERE base = 'main'
  AND EXISTS (
    SELECT 1 FROM project_repo r
    WHERE r.project_id = pr.project_id
      AND r.branch NOT IN ('', 'main')
      AND lower(replace(rtrim(r.url, '/'), '.git', '')) LIKE '%' || lower(pr.repo)
  );
"#;
