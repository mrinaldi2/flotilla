//! The owner's Stop on a bot's command (0.24.0, `commands_v1`): which
//! commands the owner stopped, so the list says so after the transcript
//! records how the command ended.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use rusqlite::params;

use super::{parse_ts, ts, Db};

impl Db {
    /// Records that `by` stopped the bot's command; a second stop of the
    /// same command keeps the first.
    pub fn record_command_stop(
        &self,
        bot_id: &str,
        command_id: &str,
        command: &str,
        by: &str,
        at: DateTime<Utc>,
    ) -> anyhow::Result<()> {
        self.lock().execute(
            "INSERT OR IGNORE INTO command_stop (bot_id, command_id, command, stopped_by, stopped_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![bot_id, command_id, command, by, ts(at)],
        )?;
        Ok(())
    }

    /// The bot's commands the owner stopped, by command id, and when.
    pub fn command_stops(&self, bot_id: &str) -> anyhow::Result<HashMap<String, DateTime<Utc>>> {
        let conn = self.lock();
        let mut stmt =
            conn.prepare("SELECT command_id, stopped_at FROM command_stop WHERE bot_id = ?1")?;
        let rows = stmt
            .query_map(params![bot_id], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows
            .into_iter()
            .map(|(id, at)| (id, parse_ts(&at)))
            .collect())
    }
}
