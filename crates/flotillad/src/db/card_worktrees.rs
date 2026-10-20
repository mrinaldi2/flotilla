//! Lighter checkouts (0.23.0): the worktrees this computer made per card
//! from a project's mirror (`card_worktree`), and the project's disk budget.

use rusqlite::{params, OptionalExtension, Row};

use super::Db;

/// A card's worktree on this computer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardWorktree {
    pub project_id: String,
    pub item_id: String,
    pub path: String,
    pub branch: String,
    /// The branch it started from.
    pub base: String,
    /// The folders it is sparse to; empty for a full checkout.
    pub paths: Vec<String>,
    /// The bot that asked for it first.
    pub bot_id: String,
    pub created_at: String,
}

const COLUMNS: &str = "project_id, item_id, path, branch, base, paths, bot_id, created_at";

fn row(r: &Row<'_>) -> rusqlite::Result<CardWorktree> {
    let paths: String = r.get(5)?;
    Ok(CardWorktree {
        project_id: r.get(0)?,
        item_id: r.get(1)?,
        path: r.get(2)?,
        branch: r.get(3)?,
        base: r.get(4)?,
        paths: serde_json::from_str(&paths).unwrap_or_default(),
        bot_id: r.get(6)?,
        created_at: r.get(7)?,
    })
}

impl Db {
    /// The project's disk budget in bytes; `None`, no budget.
    pub fn project_disk_budget(&self, project_id: &str) -> anyhow::Result<Option<u64>> {
        let bytes: Option<Option<i64>> = self
            .lock()
            .query_row(
                "SELECT disk_budget_bytes FROM project WHERE id = ?1",
                params![project_id],
                |r| r.get(0),
            )
            .optional()?;
        Ok(bytes.flatten().and_then(|b| u64::try_from(b).ok()))
    }

    pub fn set_project_disk_budget(
        &self,
        project_id: &str,
        bytes: Option<u64>,
    ) -> anyhow::Result<()> {
        let bytes = bytes.map(i64::try_from).transpose()?;
        self.lock().execute(
            "UPDATE project SET disk_budget_bytes = ?2 WHERE id = ?1",
            params![project_id, bytes],
        )?;
        Ok(())
    }

    /// Whether the owner turned the project's pull requests on in the app
    /// (0.23.1).
    pub fn project_pr_flow(&self, project_id: &str) -> anyhow::Result<bool> {
        Ok(self
            .lock()
            .query_row(
                "SELECT pr_flow FROM project WHERE id = ?1",
                params![project_id],
                |r| r.get(0),
            )
            .optional()?
            .unwrap_or(false))
    }

    pub fn set_project_pr_flow(&self, project_id: &str, on: bool) -> anyhow::Result<()> {
        self.lock().execute(
            "UPDATE project SET pr_flow = ?2 WHERE id = ?1",
            params![project_id, on],
        )?;
        Ok(())
    }

    pub fn card_worktree(
        &self,
        project_id: &str,
        item_id: &str,
    ) -> anyhow::Result<Option<CardWorktree>> {
        Ok(self
            .lock()
            .query_row(
                &format!(
                    "SELECT {COLUMNS} FROM card_worktree WHERE project_id = ?1 AND item_id = ?2"
                ),
                params![project_id, item_id],
                row,
            )
            .optional()?)
    }

    /// Every card worktree here, or the project's, oldest first.
    pub fn card_worktrees(&self, project_id: Option<&str>) -> anyhow::Result<Vec<CardWorktree>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(&format!(
            "SELECT {COLUMNS} FROM card_worktree
             WHERE ?1 IS NULL OR project_id = ?1 ORDER BY created_at, item_id"
        ))?;
        let rows = stmt
            .query_map(params![project_id], row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn add_card_worktree(&self, tree: &CardWorktree) -> anyhow::Result<()> {
        self.lock().execute(
            &format!(
                "INSERT OR REPLACE INTO card_worktree ({COLUMNS})
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"
            ),
            params![
                tree.project_id,
                tree.item_id,
                tree.path,
                tree.branch,
                tree.base,
                serde_json::to_string(&tree.paths)?,
                tree.bot_id,
                tree.created_at
            ],
        )?;
        Ok(())
    }

    /// Records the folders a sparse worktree now holds.
    pub fn set_card_worktree_paths(
        &self,
        project_id: &str,
        item_id: &str,
        paths: &[String],
    ) -> anyhow::Result<()> {
        self.lock().execute(
            "UPDATE card_worktree SET paths = ?3 WHERE project_id = ?1 AND item_id = ?2",
            params![project_id, item_id, serde_json::to_string(paths)?],
        )?;
        Ok(())
    }

    pub fn remove_card_worktree(&self, project_id: &str, item_id: &str) -> anyhow::Result<()> {
        self.lock().execute(
            "DELETE FROM card_worktree WHERE project_id = ?1 AND item_id = ?2",
            params![project_id, item_id],
        )?;
        Ok(())
    }
}
