//! What the team on demand (0.23.0) reads about a bot's work: whether it
//! has any, and the clock its tasks keep while it waits for a slot.

use bus::now;
use chrono::Duration;
use rusqlite::{params, OptionalExtension};

use super::{parse_ts, ts, Db};

/// The categories of an active column: a card of the bot's in one keeps it
/// awake. Inbox and Ready wait; Done and Cancelled are over.
const ACTIVE: &str = "('research', 'doing', 'review', 'verify', 'approval', 'deploying')";

/// The work that keeps a bot awake, for its wake at boot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenWork {
    /// A task to it is open; the card it was delegated for, if any.
    Task(Option<String>),
    /// A card of its is in an active column.
    Card(String),
}

impl Db {
    /// An open task to the bot, else a live card of its in an active column.
    pub fn bot_open_work(&self, bot_id: &str) -> anyhow::Result<Option<OpenWork>> {
        let conn = self.lock();
        let task: Option<Option<String>> = conn
            .query_row(
                "SELECT item_id FROM task WHERE to_bot_id = ?1 AND state = 'open'
                 ORDER BY created_at LIMIT 1",
                params![bot_id],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(item) = task {
            return Ok(Some(OpenWork::Task(item)));
        }
        let card: Option<String> = conn
            .query_row(
                &format!(
                    "SELECT id FROM item WHERE assignee = ?1 AND state = 'live'
                       AND category IN {ACTIVE} ORDER BY created_at LIMIT 1"
                ),
                params![bot_id],
                |r| r.get(0),
            )
            .optional()?;
        Ok(card.map(OpenWork::Card))
    }

    /// True while the bot has work: an open task, a card in an active
    /// column, or a delivery waiting for it. A bot with none may sleep.
    pub fn bot_has_work(&self, bot_id: &str) -> anyhow::Result<bool> {
        if self.bot_open_work(bot_id)?.is_some() {
            return Ok(true);
        }
        Ok(self.lock().query_row(
            "SELECT EXISTS(SELECT 1 FROM delivery WHERE bot_id = ?1
                             AND state IN ('queued', 'leased'))",
            params![bot_id],
            |r| r.get(0),
        )?)
    }

    /// Moves the deadline of each open task to the bot on by `by`: the time
    /// it waited for a slot doesn't count against it.
    pub fn extend_bot_task_deadlines(&self, bot_id: &str, by: Duration) -> anyhow::Result<usize> {
        let conn = self.lock();
        let open: Vec<(String, String)> = conn
            .prepare(
                "SELECT id, deadline_at FROM task
                 WHERE to_bot_id = ?1 AND state = 'open' AND deadline_at IS NOT NULL",
            )?
            .query_map(params![bot_id], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<_, _>>()?;
        for (id, deadline) in &open {
            conn.execute(
                "UPDATE task SET deadline_at = ?2 WHERE id = ?1",
                params![id, ts(parse_ts(deadline) + by)],
            )?;
        }
        Ok(open.len())
    }

    /// Makes the bot's waiting deliveries due now: its session just came
    /// up, and a delivery that waited long backs off up to a minute.
    pub fn deliveries_due_now(&self, bot_id: &str) -> anyhow::Result<usize> {
        let now = ts(now());
        Ok(self.lock().execute(
            "UPDATE delivery SET next_attempt_at = ?2
             WHERE bot_id = ?1 AND state = 'queued' AND next_attempt_at > ?2",
            params![bot_id, now],
        )?)
    }
}
