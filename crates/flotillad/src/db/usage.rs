//! Usage and budgets (0.24.0, `usage_v1`): the token counts read from the
//! bots' transcripts, the price table, and each project's weekly budget.

use chrono::Utc;
use rusqlite::{params, OptionalExtension, Transaction};

use super::Db;

/// Tokens of each kind.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Tokens {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
}

impl Tokens {
    pub fn total(&self) -> u64 {
        self.input + self.output + self.cache_read + self.cache_write
    }

    pub fn add(&mut self, other: &Tokens) {
        self.input += other.input;
        self.output += other.output;
        self.cache_read += other.cache_read;
        self.cache_write += other.cache_write;
    }

    /// What `self` has beyond `seen`, each kind on its own.
    fn beyond(&self, seen: &Tokens) -> Tokens {
        Tokens {
            input: self.input.saturating_sub(seen.input),
            output: self.output.saturating_sub(seen.output),
            cache_read: self.cache_read.saturating_sub(seen.cache_read),
            cache_write: self.cache_write.saturating_sub(seen.cache_write),
        }
    }

    fn max(&self, other: &Tokens) -> Tokens {
        Tokens {
            input: self.input.max(other.input),
            output: self.output.max(other.output),
            cache_read: self.cache_read.max(other.cache_read),
            cache_write: self.cache_write.max(other.cache_write),
        }
    }
}

/// One transcript entry that counts, in the order the transcript has them.
/// `key` names it across reads; `day` is its local date.
#[derive(Debug, Clone, PartialEq)]
pub enum UsageEntry {
    /// The runtime's reply, with what it used. A key read again adds only
    /// what grew since.
    Reply {
        key: String,
        day: String,
        model: String,
        tokens: Tokens,
    },
    /// A prompt that starts a turn: counted on the reply after it.
    Prompt { key: String },
    /// A turn's end: counted on the reply before it (Codex).
    TurnEnd { key: String },
}

/// One `usage_day` row.
#[derive(Debug, Clone, PartialEq)]
pub struct UsageRow {
    pub bot_id: String,
    pub project_id: String,
    pub day: String,
    pub model: String,
    pub tokens: Tokens,
    pub turns: u64,
}

pub(super) fn int(n: u64) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

pub(super) fn uint(n: i64) -> u64 {
    u64::try_from(n).unwrap_or(0)
}

impl Db {
    /// How far the bot's transcript at `path` was read.
    pub fn usage_cursor(&self, bot_id: &str, path: &str) -> anyhow::Result<u64> {
        let offset: Option<i64> = self
            .lock()
            .query_row(
                "SELECT offset FROM usage_cursor WHERE bot_id = ?1 AND path = ?2",
                params![bot_id, path],
                |r| r.get(0),
            )
            .optional()?;
        Ok(offset.map_or(0, uint))
    }

    /// Adds the entries read from `path` up to `offset` to the bot's days,
    /// under `project_id`. Whether any count changed.
    pub fn usage_apply(
        &self,
        bot_id: &str,
        project_id: &str,
        path: &str,
        offset: u64,
        entries: &[UsageEntry],
    ) -> anyhow::Result<bool> {
        let mut conn = self.lock();
        let tx = conn.transaction()?;
        let mut pending: u64 = tx
            .query_row(
                "SELECT pending_turns FROM usage_cursor WHERE bot_id = ?1 AND path = ?2",
                params![bot_id, path],
                |r| r.get::<_, i64>(0),
            )
            .optional()?
            .map_or(0, uint);
        let mut changed = false;
        let mut last: Option<(String, String)> = None;
        for entry in entries {
            match entry {
                UsageEntry::Prompt { key } => {
                    if mark_seen(&tx, bot_id, key)? {
                        pending += 1;
                    }
                }
                UsageEntry::TurnEnd { key } => {
                    if !mark_seen(&tx, bot_id, key)? {
                        continue;
                    }
                    match &last {
                        Some((day, model)) => {
                            add_row(&tx, bot_id, project_id, day, model, &Tokens::default(), 1)?;
                            changed = true;
                        }
                        None => pending += 1,
                    }
                }
                UsageEntry::Reply {
                    key,
                    day,
                    model,
                    tokens,
                } => {
                    let seen = seen_reply(&tx, bot_id, key)?;
                    let (day, delta, turns) = match &seen {
                        Some((day, before)) => (day.clone(), tokens.beyond(before), 0),
                        None => (day.clone(), *tokens, std::mem::take(&mut pending)),
                    };
                    let kept = seen.as_ref().map_or(*tokens, |(_, b)| tokens.max(b));
                    tx.execute(
                        "INSERT INTO usage_seen (bot_id, key, day, input, output, cache_read, cache_write)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                         ON CONFLICT(bot_id, key) DO UPDATE SET input = ?4, output = ?5,
                             cache_read = ?6, cache_write = ?7",
                        params![
                            bot_id, key, day, int(kept.input), int(kept.output),
                            int(kept.cache_read), int(kept.cache_write)
                        ],
                    )?;
                    if delta.total() > 0 || turns > 0 {
                        add_row(&tx, bot_id, project_id, &day, model, &delta, turns)?;
                        changed = true;
                    }
                    last = Some((day, model.clone()));
                }
            }
        }
        tx.execute(
            "INSERT INTO usage_cursor (bot_id, path, offset, pending_turns) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(bot_id, path) DO UPDATE SET offset = ?3, pending_turns = ?4",
            params![bot_id, path, int(offset), int(pending)],
        )?;
        tx.commit()?;
        Ok(changed)
    }

    /// The rows from `since` (a local date) on, of one project or bot.
    pub fn usage_rows(
        &self,
        since: &str,
        project_id: Option<&str>,
        bot_id: Option<&str>,
    ) -> anyhow::Result<Vec<UsageRow>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(
            "SELECT bot_id, project_id, day, model, input, output, cache_read, cache_write, turns
             FROM usage_day WHERE day >= ?1
               AND (?2 IS NULL OR project_id = ?2) AND (?3 IS NULL OR bot_id = ?3)
             ORDER BY day, bot_id, model",
        )?;
        let rows = stmt
            .query_map(params![since, project_id, bot_id], |r| {
                Ok(UsageRow {
                    bot_id: r.get(0)?,
                    project_id: r.get(1)?,
                    day: r.get(2)?,
                    model: r.get(3)?,
                    tokens: Tokens {
                        input: uint(r.get(4)?),
                        output: uint(r.get(5)?),
                        cache_read: uint(r.get(6)?),
                        cache_write: uint(r.get(7)?),
                    },
                    turns: uint(r.get(8)?),
                })
            })?
            .collect::<Result<_, _>>()?;
        Ok(rows)
    }

    /// Forgets the entries counted before `day`: a transcript that old is
    /// past every range.
    pub fn prune_usage_seen(&self, day: &str) -> anyhow::Result<usize> {
        Ok(self
            .lock()
            .execute("DELETE FROM usage_seen WHERE day < ?1", params![day])?)
    }
}

/// Records a prompt or a turn's end; false when it was counted before.
fn mark_seen(tx: &Transaction<'_>, bot_id: &str, key: &str) -> anyhow::Result<bool> {
    let day = Utc::now().date_naive().to_string();
    Ok(tx.execute(
        "INSERT OR IGNORE INTO usage_seen (bot_id, key, day) VALUES (?1, ?2, ?3)",
        params![bot_id, key, day],
    )? > 0)
}

fn seen_reply(
    tx: &Transaction<'_>,
    bot_id: &str,
    key: &str,
) -> anyhow::Result<Option<(String, Tokens)>> {
    Ok(tx
        .query_row(
            "SELECT day, input, output, cache_read, cache_write FROM usage_seen
             WHERE bot_id = ?1 AND key = ?2",
            params![bot_id, key],
            |r| {
                Ok((
                    r.get(0)?,
                    Tokens {
                        input: uint(r.get(1)?),
                        output: uint(r.get(2)?),
                        cache_read: uint(r.get(3)?),
                        cache_write: uint(r.get(4)?),
                    },
                ))
            },
        )
        .optional()?)
}

fn add_row(
    tx: &Transaction<'_>,
    bot_id: &str,
    project_id: &str,
    day: &str,
    model: &str,
    t: &Tokens,
    turns: u64,
) -> anyhow::Result<()> {
    tx.execute(
        "INSERT INTO usage_day (bot_id, day, model, project_id, input, output, cache_read,
                                cache_write, turns)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT(bot_id, day, model) DO UPDATE SET
             input = input + ?5, output = output + ?6, cache_read = cache_read + ?7,
             cache_write = cache_write + ?8, turns = turns + ?9",
        params![
            bot_id,
            day,
            model,
            project_id,
            int(t.input),
            int(t.output),
            int(t.cache_read),
            int(t.cache_write),
            int(turns)
        ],
    )?;
    Ok(())
}
