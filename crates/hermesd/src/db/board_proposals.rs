//! Structure changes a lead made or proposed (0.19.0 K4): open proposals the
//! owner rules on, and the lead's applied changes, kept for the board's
//! "changed by the lead" list.

use bus::now;
use rusqlite::{params, OptionalExtension};

use super::{parse_ts, ts, Db};

#[derive(Debug, Clone, PartialEq)]
pub struct BoardProposal {
    pub id: String,
    pub project_id: String,
    pub board_id: Option<String>,
    /// The structure request as proto JSON.
    pub request: String,
    pub summary: String,
    pub proposed_by: String,
    pub decision_id: Option<String>,
    pub state: String,
    pub note: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

const COLUMNS: &str =
    "id, project_id, board_id, request, summary, proposed_by, decision_id, state, note, created_at";

fn from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<BoardProposal> {
    Ok(BoardProposal {
        id: r.get(0)?,
        project_id: r.get(1)?,
        board_id: r.get(2)?,
        request: r.get(3)?,
        summary: r.get(4)?,
        proposed_by: r.get(5)?,
        decision_id: r.get(6)?,
        state: r.get(7)?,
        note: r.get(8)?,
        created_at: parse_ts(&r.get::<_, String>(9)?),
    })
}

impl Db {
    /// Records a lead's change: `open` to propose it, `applied` when it was
    /// made directly.
    pub fn add_board_proposal(
        &self,
        project_id: &str,
        board_id: Option<&str>,
        request: &str,
        summary: &str,
        proposed_by: &str,
        state: &str,
    ) -> anyhow::Result<BoardProposal> {
        let id = uuid::Uuid::new_v4().to_string();
        let conn = self.lock();
        conn.execute(
            "INSERT INTO board_proposal(id, project_id, board_id, request, summary, proposed_by,
                                        state, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                id,
                project_id,
                board_id,
                request,
                summary,
                proposed_by,
                state,
                ts(now())
            ],
        )?;
        Ok(conn.query_row(
            &format!("SELECT {COLUMNS} FROM board_proposal WHERE id = ?1"),
            params![id],
            from_row,
        )?)
    }

    pub fn set_proposal_decision(&self, id: &str, decision_id: &str) -> anyhow::Result<()> {
        self.lock().execute(
            "UPDATE board_proposal SET decision_id = ?2 WHERE id = ?1",
            params![id, decision_id],
        )?;
        Ok(())
    }

    pub fn board_proposal(&self, id: &str) -> anyhow::Result<Option<BoardProposal>> {
        Ok(self
            .lock()
            .query_row(
                &format!("SELECT {COLUMNS} FROM board_proposal WHERE id = ?1"),
                params![id],
                from_row,
            )
            .optional()?)
    }

    /// Moves an open proposal to `state`; false when it was no longer open.
    pub fn rule_board_proposal(
        &self,
        id: &str,
        state: &str,
        note: Option<&str>,
    ) -> anyhow::Result<bool> {
        Ok(self.lock().execute(
            "UPDATE board_proposal SET state = ?2, note = ?3, ruled_at = ?4
             WHERE id = ?1 AND state = 'open'",
            params![id, state, note, ts(now())],
        )? == 1)
    }

    /// A project's proposals and the lead's changes, newest first.
    pub fn board_proposals(&self, project_id: &str) -> anyhow::Result<Vec<BoardProposal>> {
        Ok(self
            .lock()
            .prepare(&format!(
                "SELECT {COLUMNS} FROM board_proposal WHERE project_id = ?1
                 ORDER BY created_at DESC"
            ))?
            .query_map(params![project_id], from_row)?
            .collect::<Result<_, _>>()?)
    }
}
