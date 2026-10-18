//! Which live cards use which artifacts (0.22.0): attachments kept in the
//! artifacts folder and `artifact` links, as stored; and the loose artifacts
//! the owner kept.

use std::collections::HashSet;

use bus::now;
use rusqlite::params;

use super::board_tx::BoardTx;
use super::{ts, Db};

impl BoardTx<'_> {
    /// `(card, artifact)` for each artifact a live card of the project
    /// names, in card order: an attachment's path (relative to the
    /// artifacts folder) and an `artifact` link's target (as written,
    /// relative or absolute). Detached attachments don't count.
    pub fn artifact_uses(&self, project_id: &str) -> anyhow::Result<Vec<(String, String)>> {
        let rows = self
            .conn
            .prepare(
                "SELECT i.seq, a.item_id, a.path FROM item_attachment a
                   JOIN item i ON i.id = a.item_id
                  WHERE i.project_id = ?1 AND i.state = 'live'
                    AND a.removed_at IS NULL AND a.path != ''
                 UNION ALL
                 SELECT i.seq, l.item_id, l.ref FROM item_link l
                   JOIN item i ON i.id = l.item_id
                  WHERE i.project_id = ?1 AND i.state = 'live' AND l.kind = 'artifact'
                 ORDER BY 1",
            )?
            .query_map(params![project_id], |r| Ok((r.get(1)?, r.get(2)?)))?
            .collect::<Result<_, _>>()?;
        Ok(rows)
    }
}

impl Db {
    /// Marks the artifact at `path` kept (`keep`) or not; true when that
    /// changed anything.
    pub fn set_artifact_kept(
        &self,
        project_id: &str,
        path: &str,
        keep: bool,
    ) -> anyhow::Result<bool> {
        let changed = if keep {
            self.lock().execute(
                "INSERT OR IGNORE INTO artifact_keep(project_id, path, kept_at) VALUES (?1, ?2, ?3)",
                params![project_id, path, ts(now())],
            )?
        } else {
            self.lock().execute(
                "DELETE FROM artifact_keep WHERE project_id = ?1 AND path = ?2",
                params![project_id, path],
            )?
        };
        Ok(changed > 0)
    }

    /// The paths of the project's artifacts the owner kept.
    pub fn kept_artifacts(&self, project_id: &str) -> anyhow::Result<HashSet<String>> {
        let conn = self.lock();
        let rows = conn
            .prepare("SELECT path FROM artifact_keep WHERE project_id = ?1")?
            .query_map(params![project_id], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        Ok(rows)
    }
}
