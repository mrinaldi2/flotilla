//! Which live cards use which artifacts (0.22.0): attachments kept in the
//! artifacts folder and `artifact` links, as stored.

use rusqlite::params;

use super::board_tx::BoardTx;

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
