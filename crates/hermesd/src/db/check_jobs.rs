//! Check jobs (H-283, H-261 §7): each hand-over of a queued check to the
//! daemon runner of one computer. Open jobs are what the per-machine cap
//! counts; a job ends when its check has a final result.

use bus::now;
use rusqlite::{params, OptionalExtension, Row};

use super::board_tx::BoardTx;
use super::checks::{check_row, COLUMNS};
use super::{parse_ts, ts};
use crate::prs::check_model::{CheckRun, Unroutable};
use crate::prs::check_route::{is_heavy, needs_of};

/// Why a job ran: the check's first run, the one automatic retry after an
/// `error`, or a re-run someone asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobCause {
    First,
    Auto,
    Rerun,
}

impl JobCause {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::First => "first",
            Self::Auto => "auto",
            Self::Rerun => "rerun",
        }
    }

    fn parse(s: &str) -> Self {
        match s {
            "auto" => Self::Auto,
            "rerun" => Self::Rerun,
            _ => Self::First,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CheckJob {
    pub id: String,
    pub check_id: String,
    pub project_id: String,
    pub machine: String,
    pub cause: JobCause,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub ended_at: Option<chrono::DateTime<chrono::Utc>>,
}

const JOB_COLUMNS: &str = "id, check_id, project_id, machine, cause, created_at, ended_at";

fn job_row(r: &Row<'_>) -> rusqlite::Result<CheckJob> {
    Ok(CheckJob {
        id: r.get(0)?,
        check_id: r.get(1)?,
        project_id: r.get(2)?,
        machine: r.get(3)?,
        cause: JobCause::parse(&r.get::<_, String>(4)?),
        created_at: parse_ts(&r.get::<_, String>(5)?),
        ended_at: r.get::<_, Option<String>>(6)?.as_deref().map(parse_ts),
    })
}

impl BoardTx<'_> {
    /// The project's queued checks no runner has been asked to run yet,
    /// oldest first. A check with nothing to run (the `checks.toml` check)
    /// is never one.
    pub fn undispatched_checks(&self, project_id: &str) -> anyhow::Result<Vec<CheckRun>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {COLUMNS} FROM check_run c
             WHERE project_id = ?1 AND result = 'queued' AND runner IS NULL AND run != ''
               AND NOT EXISTS (SELECT 1 FROM check_job j
                               WHERE j.check_id = c.id AND j.ended_at IS NULL
                                 AND j.machine != '')
             ORDER BY queued_at, name"
        ))?;
        let rows = stmt
            .query_map(params![project_id], check_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// The check with this id.
    pub fn check_by_id(&self, id: &str) -> anyhow::Result<Option<CheckRun>> {
        Ok(self
            .conn
            .query_row(
                &format!("SELECT {COLUMNS} FROM check_run WHERE id = ?1"),
                params![id],
                check_row,
            )
            .optional()?)
    }

    /// Open jobs on a computer, every project's: what its cap counts.
    pub fn open_jobs_on(&self, machine: &str) -> anyhow::Result<usize> {
        let n: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM check_job WHERE machine = ?1 AND ended_at IS NULL",
            params![machine],
            |r| r.get(0),
        )?;
        Ok(n as usize)
    }

    /// Open heavy jobs on a computer, every project's: what its heavy slot
    /// counts (H-334).
    pub fn open_heavy_on(&self, machine: &str) -> anyhow::Result<usize> {
        let mut stmt = self.conn.prepare(
            "SELECT c.needs, c.run,
                    (SELECT heavy FROM check_heavy h WHERE h.check_id = c.id)
             FROM check_job j JOIN check_run c ON c.id = j.check_id
             WHERE j.machine = ?1 AND j.ended_at IS NULL",
        )?;
        let rows = stmt.query_map(params![machine], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<bool>>(2)?,
            ))
        })?;
        let mut n = 0;
        for row in rows {
            let (needs, run, said) = row?;
            let needs: Vec<String> = serde_json::from_str(&needs).unwrap_or_default();
            n += usize::from(is_heavy(&needs_of(&needs, &run), said));
        }
        Ok(n)
    }

    /// A project's open jobs.
    pub fn open_jobs(&self, project_id: &str) -> anyhow::Result<Vec<CheckJob>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {JOB_COLUMNS} FROM check_job
             WHERE project_id = ?1 AND ended_at IS NULL AND machine != ''
             ORDER BY created_at"
        ))?;
        let rows = stmt
            .query_map(params![project_id], job_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Hands the check to `machine`'s runner: the job a re-run or a retry
    /// left pending, else a first one. Returns the job's id.
    pub fn assign_check_job(&self, check: &CheckRun, machine: &str) -> anyhow::Result<String> {
        if let Some((id, _)) = self.pending_job(&check.id)? {
            self.conn.execute(
                "UPDATE check_job SET machine = ?2 WHERE id = ?1",
                params![id, machine],
            )?;
            return Ok(id);
        }
        self.insert_job(check, machine, JobCause::First)
    }

    /// Takes back a job its computer wouldn't start: a first one goes, a
    /// re-run or a retry waits again to be routed.
    pub fn unassign_check_job(&self, job_id: &str) -> anyhow::Result<()> {
        self.conn.execute(
            "DELETE FROM check_job WHERE id = ?1 AND cause = 'first'",
            params![job_id],
        )?;
        self.conn.execute(
            "UPDATE check_job SET machine = '' WHERE id = ?1",
            params![job_id],
        )?;
        Ok(())
    }

    /// The job with this id.
    pub fn check_job(&self, id: &str) -> anyhow::Result<Option<CheckJob>> {
        Ok(self
            .conn
            .query_row(
                &format!("SELECT {JOB_COLUMNS} FROM check_job WHERE id = ?1"),
                params![id],
                job_row,
            )
            .optional()?)
    }

    /// A job for the check's next run, routed when the dispatcher gets to it.
    pub fn pend_check_job(&self, check: &CheckRun, cause: JobCause) -> anyhow::Result<()> {
        self.insert_job(check, "", cause).map(|_| ())
    }

    fn insert_job(
        &self,
        check: &CheckRun,
        machine: &str,
        cause: JobCause,
    ) -> anyhow::Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        self.conn.execute(
            "INSERT INTO check_job(id, check_id, project_id, machine, cause, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                id,
                check.id,
                check.project_id,
                machine,
                cause.as_str(),
                ts(now())
            ],
        )?;
        Ok(id)
    }

    fn pending_job(&self, check_id: &str) -> anyhow::Result<Option<(String, JobCause)>> {
        Ok(self
            .conn
            .query_row(
                "SELECT id, cause FROM check_job
                 WHERE check_id = ?1 AND machine = '' AND ended_at IS NULL
                 ORDER BY created_at DESC LIMIT 1",
                params![check_id],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        JobCause::parse(&r.get::<_, String>(1)?),
                    ))
                },
            )
            .optional()?)
    }

    /// Ends the check's routed jobs; a pending one waits for its run.
    pub fn end_check_jobs(&self, check_id: &str) -> anyhow::Result<()> {
        self.conn.execute(
            "UPDATE check_job SET ended_at = ?2
             WHERE check_id = ?1 AND ended_at IS NULL AND machine != ''",
            params![check_id, ts(now())],
        )?;
        Ok(())
    }

    /// Why the check's latest job ran, if it ever had one.
    pub fn last_job_cause(&self, check_id: &str) -> anyhow::Result<Option<JobCause>> {
        let cause: Option<String> = self
            .conn
            .query_row(
                "SELECT cause FROM check_job WHERE check_id = ?1 AND machine != ''
                 ORDER BY created_at DESC, rowid DESC LIMIT 1",
                params![check_id],
                |r| r.get(0),
            )
            .optional()?;
        Ok(cause.as_deref().map(JobCause::parse))
    }

    /// Undoes a dispatch its computer refused: queued again, no runner.
    pub fn undispatch_check(&self, id: &str) -> anyhow::Result<()> {
        self.conn.execute(
            "UPDATE check_run SET result = 'queued', runner = NULL, ran_on = NULL,
                tool_versions = '{}', started_at = NULL
             WHERE id = ?1 AND result = 'running'",
            params![id],
        )?;
        self.note_check_id(id)
    }

    /// Puts a check back in the queue for a new run: no runner, no result.
    /// Its log, if it had one, stays published under the same name until
    /// the new run replaces it.
    pub fn requeue_check(&self, id: &str, note: &str) -> anyhow::Result<()> {
        self.conn.execute(
            "UPDATE check_run SET result = 'queued', runner = NULL, ran_on = NULL,
                log_artifact = NULL, tool_versions = '{}', note = ?2, queued_at = ?3,
                started_at = NULL, finished_at = NULL
             WHERE id = ?1",
            params![id, note, ts(now())],
        )?;
        self.note_check_id(id)
    }

    /// Why a queued check still waits, shown with it. It isn't unroutable
    /// (any more): it waits for something that comes, or it routed.
    pub fn set_check_note(&self, id: &str, note: Option<&str>) -> anyhow::Result<bool> {
        let n = self.conn.execute(
            "UPDATE check_run SET note = ?2 WHERE id = ?1 AND note IS NOT ?2",
            params![id, note],
        )? + self.conn.execute(
            "DELETE FROM check_unroutable WHERE check_id = ?1",
            params![id],
        )?;
        if n > 0 {
            self.note_check_id(id)?;
        }
        Ok(n > 0)
    }

    /// No computer can run the queued check (H-318): `note` says why, and
    /// `unroutable_since` keeps the first time it was found so.
    pub fn set_check_unroutable(&self, id: &str, note: &str) -> anyhow::Result<bool> {
        let n = self.conn.execute(
            "UPDATE check_run SET note = ?2
             WHERE id = ?1 AND result = 'queued' AND note IS NOT ?2",
            params![id, note],
        )? + self.conn.execute(
            "INSERT INTO check_unroutable(check_id, since)
             SELECT id, ?2 FROM check_run WHERE id = ?1 AND result = 'queued'
             ON CONFLICT(check_id) DO NOTHING",
            params![id, ts(now())],
        )?;
        if n > 0 {
            self.note_check_id(id)?;
        }
        Ok(n > 0)
    }

    /// The project's required checks on an open PR's head that no computer
    /// could run since `before` or earlier (H-318): one per check name, its
    /// oldest such time, its note and the newest PR it holds up.
    pub fn unroutable_checks(
        &self,
        project_id: &str,
        before: chrono::DateTime<chrono::Utc>,
    ) -> anyhow::Result<Vec<Unroutable>> {
        let mut stmt = self.conn.prepare(
            "SELECT c.name, MIN(u.since), MAX(c.note), MAX(p.number)
             FROM check_run c
             JOIN check_unroutable u ON u.check_id = c.id
             JOIN pr p
               ON p.project_id = c.project_id AND p.head_sha = c.sha AND p.state = 'open'
             WHERE c.project_id = ?1 AND c.required = 1 AND c.result = 'queued'
             GROUP BY c.name ORDER BY c.name",
        )?;
        let rows = stmt
            .query_map(params![project_id], |r| {
                Ok(Unroutable {
                    name: r.get(0)?,
                    since: parse_ts(&r.get::<_, String>(1)?),
                    note: r.get::<_, Option<String>>(2)?.unwrap_or_default(),
                    pr: r.get(3)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows.into_iter().filter(|r| r.since <= before).collect())
    }

    /// The tools of every computer that reported any, with when it last did.
    pub fn all_machine_tools(
        &self,
    ) -> anyhow::Result<Vec<(String, std::collections::BTreeMap<String, String>)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT machine, tool, version FROM machine_tool ORDER BY machine")?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut out: Vec<(String, std::collections::BTreeMap<String, String>)> = Vec::new();
        for (machine, tool, version) in rows {
            match out.last_mut() {
                Some((m, tools)) if *m == machine => {
                    tools.insert(tool, version);
                }
                _ => out.push((machine, [(tool, version)].into())),
            }
        }
        Ok(out)
    }
}

impl BoardTx<'_> {
    /// The card and author of the newest PR whose head is `sha`.
    pub fn pr_of_head(
        &self,
        project_id: &str,
        sha: &str,
    ) -> anyhow::Result<Option<(String, String, u32)>> {
        Ok(self
            .conn
            .query_row(
                "SELECT item_id, author, number FROM pr WHERE project_id = ?1 AND head_sha = ?2
                 ORDER BY opened_at DESC LIMIT 1",
                params![project_id, sha],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?)
    }
}
