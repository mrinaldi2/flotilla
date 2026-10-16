//! What a project delete asked of each linked computer (0.19.0), kept until
//! it answers or counts as not confirming; see `project_trash::peer`. And
//! the deleted projects the cleanup of the past looks through.

use bus::Project;
use chrono::{DateTime, Utc};
use rusqlite::{params, Row};
use serde::Serialize;

use super::{parse_ts, ts, Db};

/// One linked computer asked to trash its half of a deleted project.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TrashOrder {
    pub id: String,
    /// The deleted project, this computer's id.
    pub project_id: String,
    pub project_name: String,
    pub peer_id: String,
    /// The linked project on that computer, its id.
    pub remote_project_id: String,
    /// `pending`, `confirmed` or `unconfirmed`.
    pub state: String,
    pub attempts: i64,
    pub sent_at: Option<DateTime<Utc>>,
    /// Its answer, as it sent it.
    pub outcome: Option<serde_json::Value>,
    pub moved_bytes: i64,
    pub created_at: DateTime<Utc>,
}

const COLUMNS: &str = "id, project_id, project_name, peer_id, remote_project_id, state, attempts, \
                       sent_at, outcome, moved_bytes, created_at";

fn row(r: &Row<'_>) -> rusqlite::Result<TrashOrder> {
    let sent: Option<String> = r.get(7)?;
    let outcome: Option<String> = r.get(8)?;
    Ok(TrashOrder {
        id: r.get(0)?,
        project_id: r.get(1)?,
        project_name: r.get(2)?,
        peer_id: r.get(3)?,
        remote_project_id: r.get(4)?,
        state: r.get(5)?,
        attempts: r.get(6)?,
        sent_at: sent.as_deref().map(parse_ts),
        outcome: outcome.and_then(|o| serde_json::from_str(&o).ok()),
        moved_bytes: r.get(9)?,
        created_at: parse_ts(&r.get::<_, String>(10)?),
    })
}

impl Db {
    /// Keeps the ask for `peer_id` until it answers.
    pub fn insert_trash_order(
        &self,
        project: &Project,
        peer_id: &str,
        remote_project_id: &str,
    ) -> anyhow::Result<TrashOrder> {
        let now = ts(Utc::now());
        let id = uuid::Uuid::new_v4().to_string();
        let conn = self.lock();
        conn.execute(
            "INSERT INTO project_trash_order(id, project_id, project_name, peer_id,
                 remote_project_id, state, attempts, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 'pending', 0, ?6, ?6)",
            params![
                id,
                project.id,
                Db::display_project_name(project),
                peer_id,
                remote_project_id,
                now
            ],
        )?;
        Ok(conn.query_row(
            &format!("SELECT {COLUMNS} FROM project_trash_order WHERE id = ?1"),
            params![id],
            row,
        )?)
    }

    pub fn trash_order(&self, id: &str) -> anyhow::Result<Option<TrashOrder>> {
        use rusqlite::OptionalExtension;
        Ok(self
            .lock()
            .query_row(
                &format!("SELECT {COLUMNS} FROM project_trash_order WHERE id = ?1"),
                params![id],
                row,
            )
            .optional()?)
    }

    /// The asks in `state`, for `peer_id` or every peer, oldest first.
    pub fn trash_orders(
        &self,
        state: &str,
        peer_id: Option<&str>,
    ) -> anyhow::Result<Vec<TrashOrder>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(&format!(
            "SELECT {COLUMNS} FROM project_trash_order
             WHERE state = ?1 AND (?2 IS NULL OR peer_id = ?2) ORDER BY created_at"
        ))?;
        let rows = stmt
            .query_map(params![state, peer_id], row)?
            .collect::<Result<_, _>>()?;
        Ok(rows)
    }

    /// The ask went out over a live link once more.
    pub fn trash_order_sent(&self, id: &str) -> anyhow::Result<()> {
        let now = ts(Utc::now());
        self.lock().execute(
            "UPDATE project_trash_order SET attempts = attempts + 1, sent_at = ?2, updated_at = ?2
             WHERE id = ?1 AND state = 'pending'",
            params![id, now],
        )?;
        Ok(())
    }

    /// The ask is over: `confirmed` with the computer's answer, or
    /// `unconfirmed`. False when it already was, so only one caller says so.
    pub fn end_trash_order(
        &self,
        id: &str,
        state: &str,
        outcome: Option<&serde_json::Value>,
        moved_bytes: i64,
    ) -> anyhow::Result<bool> {
        let outcome = outcome.map(serde_json::Value::to_string);
        let changed = self.lock().execute(
            "UPDATE project_trash_order SET state = ?2, outcome = ?3, moved_bytes = ?4,
                 updated_at = ?5
             WHERE id = ?1 AND state = 'pending'",
            params![id, state, outcome, moved_bytes, ts(Utc::now())],
        )?;
        Ok(changed == 1)
    }

    /// Every deleted project, most recently deleted first.
    pub fn list_deleted_projects(&self) -> anyhow::Result<Vec<Project>> {
        let ids: Vec<String> = {
            let conn = self.lock();
            let mut stmt = conn.prepare(
                "SELECT id FROM project WHERE deleted_at IS NOT NULL ORDER BY deleted_at DESC",
            )?;
            let rows = stmt.query_map([], |r| r.get(0))?;
            rows.collect::<Result<_, _>>()?
        };
        let mut out = Vec::new();
        for id in ids {
            if let Some(project) = self.get_project(&id)? {
                out.push(project);
            }
        }
        Ok(out)
    }
}
