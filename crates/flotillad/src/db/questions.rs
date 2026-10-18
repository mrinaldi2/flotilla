//! Questions for the owner (0.22.0): the `question` rows beside their
//! decisions (migration 074). A question is open while its decision is open
//! or held, answered once the owner's ruling is in, and dismissed when the
//! owner withdrew it with a reason.

use std::collections::HashMap;

use rusqlite::{params, Connection, OptionalExtension};

use super::{ts, BoardTx, Db};

/// A decision's card, plan node and blocking flag.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Anchor {
    pub item_id: Option<String>,
    pub plan_node_id: Option<String>,
    pub blocking: bool,
}

/// What only a question has, beside its decision.
#[derive(Debug, Clone, PartialEq)]
pub struct QuestionRow {
    pub decision_id: String,
    pub project_id: String,
    pub item_id: Option<String>,
    pub plan_node_id: Option<String>,
    pub blocking: bool,
    pub comment_id: Option<String>,
    pub written_to: Option<String>,
    pub dismissed_reason: Option<String>,
    pub version: u64,
}

/// A new question's row; its decision is already in.
pub struct NewQuestion<'a> {
    pub decision_id: &'a str,
    pub project_id: &'a str,
    pub item_id: Option<&'a str>,
    pub plan_node_id: Option<&'a str>,
    pub blocking: bool,
}

/// Which questions to list.
#[derive(Debug, Clone, Default)]
pub struct QuestionFilter<'a> {
    /// Left out when a card or node already names it.
    pub project_id: Option<&'a str>,
    pub item_id: Option<&'a str>,
    /// The node's own questions and its cards'.
    pub plan_node_id: Option<&'a str>,
    /// `Some(true)` open ones, `Some(false)` the rest.
    pub open: Option<bool>,
    /// From the last page: `<rank>|<created_at>|<id>`.
    pub before: Option<&'a str>,
    pub limit: u32,
}

/// The decision states a question is open in.
const OPEN: &str = "d.state IN ('open', 'held')";

const COLS: &str = "q.decision_id, q.project_id, q.item_id, q.plan_node_id, q.blocking, \
                    q.comment_id, q.written_to, q.dismissed_reason, q.version";

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<QuestionRow> {
    Ok(QuestionRow {
        decision_id: r.get(0)?,
        project_id: r.get(1)?,
        item_id: r.get(2)?,
        plan_node_id: r.get(3)?,
        blocking: r.get(4)?,
        comment_id: r.get(5)?,
        written_to: r.get(6)?,
        dismissed_reason: r.get(7)?,
        version: r.get::<_, i64>(8)? as u64,
    })
}

impl Db {
    /// The bot's decisions and questions still waiting on the owner, which
    /// count toward `MAX_OPEN_DECISIONS_PER_BOT`.
    pub fn open_asked_by(&self, bot_id: &str) -> anyhow::Result<i64> {
        Ok(self.lock().query_row(
            "SELECT count(*) FROM decision
              WHERE raised_by_bot_id = ?1 AND state IN ('open', 'answered', 'held')",
            params![bot_id],
            |r| r.get(0),
        )?)
    }

    pub fn insert_question(&self, new: &NewQuestion<'_>) -> anyhow::Result<()> {
        self.lock().execute(
            "INSERT INTO question(decision_id, project_id, item_id, plan_node_id, blocking)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                new.decision_id,
                new.project_id,
                new.item_id,
                new.plan_node_id,
                new.blocking
            ],
        )?;
        Ok(())
    }

    pub fn question_row(&self, id: &str) -> anyhow::Result<Option<QuestionRow>> {
        Ok(self
            .lock()
            .query_row(
                &format!("SELECT {COLS} FROM question q WHERE q.decision_id = ?1"),
                params![id],
                row,
            )
            .optional()?)
    }

    /// Each decision's card, node and blocking flag: a question's, or the
    /// card `raise_decision {item}` linked.
    pub fn decision_anchors(&self, ids: &[String]) -> anyhow::Result<HashMap<String, Anchor>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(
            "SELECT d.id, coalesce(q.item_id, d.item_id), q.plan_node_id, coalesce(q.blocking, 0)
               FROM decision d LEFT JOIN question q ON q.decision_id = d.id
              WHERE d.id = ?1",
        )?;
        let mut out = HashMap::new();
        for id in ids {
            let found = stmt
                .query_row(params![id], |r| {
                    Ok(Anchor {
                        item_id: r.get(1)?,
                        plan_node_id: r.get(2)?,
                        blocking: r.get(3)?,
                    })
                })
                .optional()?;
            if let Some(anchor) = found.filter(|a| *a != Anchor::default()) {
                out.insert(id.clone(), anchor);
            }
        }
        Ok(out)
    }

    /// A page of question ids, open ones first, then newest first; and the
    /// cursor for the next page while there is one.
    pub fn question_ids(
        &self,
        f: &QuestionFilter<'_>,
    ) -> anyhow::Result<(Vec<String>, Option<String>)> {
        let limit = match f.limit {
            0 => 50,
            n => n.min(200),
        };
        let rank = format!("(CASE WHEN {OPEN} THEN 0 ELSE 1 END)");
        let mut sql = format!(
            "SELECT d.id, {rank}, d.created_at FROM question q JOIN decision d ON d.id = q.decision_id
              WHERE (?1 IS NULL OR q.project_id = ?1)
                AND (?2 IS NULL OR q.item_id = ?2)
                AND (?3 IS NULL OR q.plan_node_id = ?3
                     OR q.item_id IN (SELECT id FROM item WHERE plan_node_id = ?3))"
        );
        match f.open {
            Some(true) => sql.push_str(&format!(" AND {OPEN}")),
            Some(false) => sql.push_str(&format!(" AND NOT {OPEN}")),
            None => {}
        }
        let cursor = f.before.and_then(parse_cursor);
        if cursor.is_some() {
            sql.push_str(&format!(
                " AND ({rank} > ?4 OR ({rank} = ?4 AND (d.created_at < ?5
                      OR (d.created_at = ?5 AND d.id < ?6))))"
            ));
        }
        sql.push_str(&format!(
            " ORDER BY {rank}, d.created_at DESC, d.id DESC LIMIT ?7"
        ));
        let (r, at, id) = cursor.unwrap_or_default();
        let conn = self.lock();
        let rows: Vec<(String, i64, String)> = conn
            .prepare(&sql)?
            .query_map(
                params![
                    f.project_id,
                    f.item_id,
                    f.plan_node_id,
                    r,
                    at,
                    id,
                    limit + 1
                ],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )?
            .collect::<Result<_, _>>()?;
        let more = rows.len() > limit as usize;
        let page: Vec<_> = rows.into_iter().take(limit as usize).collect();
        let next = match (more, page.last()) {
            (true, Some((id, rank, at))) => Some(format!("{rank}|{at}|{id}")),
            _ => None,
        };
        Ok((page.into_iter().map(|(id, _, _)| id).collect(), next))
    }

    /// Bump the question's version if it is still `expected`; false when it
    /// changed since.
    pub fn bump_question(&self, id: &str, expected: u64) -> anyhow::Result<bool> {
        Ok(self.lock().execute(
            "UPDATE question SET version = version + 1 WHERE decision_id = ?1 AND version = ?2",
            params![id, expected as i64],
        )? == 1)
    }

    pub fn set_asked_comment(&self, id: &str, comment_id: &str) -> anyhow::Result<()> {
        self.lock().execute(
            "UPDATE question SET comment_id = ?2 WHERE decision_id = ?1",
            params![id, comment_id],
        )?;
        Ok(())
    }

    pub fn set_question_written(&self, id: &str, path: &str) -> anyhow::Result<()> {
        self.lock().execute(
            "UPDATE question SET written_to = ?2, version = version + 1 WHERE decision_id = ?1",
            params![id, path],
        )?;
        Ok(())
    }

    /// The owner dismisses an open question: its decision is withdrawn with
    /// the reason. False when it is no longer open or changed since
    /// `expected`.
    pub fn dismiss_question(
        &self,
        id: &str,
        expected: u64,
        reason: &str,
        by: &str,
    ) -> anyhow::Result<bool> {
        let mut conn = self.lock();
        let tx = conn.transaction()?;
        let current: Option<i64> = tx
            .query_row(
                "SELECT version FROM question WHERE decision_id = ?1",
                params![id],
                |r| r.get(0),
            )
            .optional()?;
        if current != Some(expected as i64) {
            return Ok(false);
        }
        let changed = tx.execute(
            "UPDATE decision SET state = 'withdrawn', withdrawn_reason = ?2, edited_at = ?3,
                    edited_by = ?4
              WHERE id = ?1 AND state IN ('open', 'held')",
            params![id, reason, ts(chrono::Utc::now()), by],
        )?;
        if changed == 0 {
            return Ok(false);
        }
        tx.execute(
            "UPDATE question SET dismissed_reason = ?2 WHERE decision_id = ?1",
            params![id, reason],
        )?;
        tx.commit()?;
        Ok(true)
    }
}

fn parse_cursor(s: &str) -> Option<(i64, String, String)> {
    let mut parts = s.splitn(3, '|');
    let rank = parts.next()?.parse().ok()?;
    Some((rank, parts.next()?.to_string(), parts.next()?.to_string()))
}

/// The card's open questions.
pub(super) fn open_on_item(conn: &Connection, item_id: &str) -> rusqlite::Result<u32> {
    conn.query_row(
        &format!(
            "SELECT count(*) FROM question q JOIN decision d ON d.id = q.decision_id
              WHERE q.item_id = ?1 AND {OPEN}"
        ),
        params![item_id],
        |r| r.get(0),
    )
}

/// A node's open questions, its own and its cards', and how many of them
/// block it.
pub(super) fn open_on_node(conn: &Connection, node_id: &str) -> rusqlite::Result<(u32, u32)> {
    conn.query_row(
        &format!(
            "SELECT count(*), coalesce(sum(q.blocking), 0)
               FROM question q JOIN decision d ON d.id = q.decision_id
              WHERE (q.plan_node_id = ?1
                     OR q.item_id IN (SELECT id FROM item WHERE plan_node_id = ?1))
                AND {OPEN}"
        ),
        params![node_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
}

impl BoardTx<'_> {
    /// The node's open blocking questions, as (id, text): its research
    /// doesn't close and its gate can't be submitted while there are any.
    pub fn blocking_questions(&self, node_id: &str) -> anyhow::Result<Vec<(String, String)>> {
        Ok(self
            .conn
            .prepare(&format!(
                "SELECT d.id, d.title FROM question q JOIN decision d ON d.id = q.decision_id
                  WHERE q.blocking = 1 AND {OPEN}
                    AND (q.plan_node_id = ?1
                         OR q.item_id IN (SELECT id FROM item WHERE plan_node_id = ?1))
                  ORDER BY d.created_at"
            ))?
            .query_map(params![node_id], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<_, _>>()?)
    }

    pub fn questions_open_on_item(&self, item_id: &str) -> anyhow::Result<u32> {
        Ok(open_on_item(self.conn, item_id)?)
    }

    pub fn questions_open_on_node(&self, node_id: &str) -> anyhow::Result<(u32, u32)> {
        Ok(open_on_node(self.conn, node_id)?)
    }
}
