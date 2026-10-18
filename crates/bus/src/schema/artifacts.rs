//! Artifacts by kind (0.22.0): the loose artifacts the owner chose to keep.
//! Named rather than numbered (ARCH-R1); safe to run again.

/// One row per artifact the owner kept (`artifact_keep`), by its
/// `/`-separated path in the project's artifacts folder. A kept file leaves
/// the tidy-up list; unkeeping it deletes the row.
pub(super) const MIGRATION_ARTIFACT_KEEP: &str = r#"
CREATE TABLE IF NOT EXISTS artifact_keep (
    project_id TEXT NOT NULL,
    path       TEXT NOT NULL,
    kept_at    TEXT NOT NULL,
    PRIMARY KEY (project_id, path)
);
"#;
