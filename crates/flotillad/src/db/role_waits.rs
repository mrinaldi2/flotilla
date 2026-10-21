//! The board runs the SDLC (0.24.0, `board_flow_v1`): a column's on-entry
//! assignment, and the cards waiting for a role nobody holds.
//!
//! An on-entry assignment is an `assigned` event with field `on_entry` and
//! the role as its note, made by the daemon whoever moved the card. A
//! Tester given the card that way checked it, it didn't work on it, so the
//! `independent` rule still lets it close the card (`Context::checker`).

use bus::now;
use rusqlite::{params, OptionalExtension};

use super::board_edit::versioned;
use super::board_items::{record, Event, Write};
use super::board_tx::BoardTx;
use super::{ts, Actor, Db};
use crate::kanban::model::{Item, ItemEventKind, Role};

/// The `field` of an on-entry assignment's event.
pub(crate) const ON_ENTRY: &str = "on_entry";

/// A card that entered a column whose role nobody held.
#[derive(Debug, Clone, PartialEq)]
pub struct RoleWait {
    pub item_id: String,
    pub board_id: String,
    pub column_key: String,
    pub role: Role,
    pub since: chrono::DateTime<chrono::Utc>,
}

impl BoardTx<'_> {
    /// Assign the card to `bot` because it entered a column that assigns
    /// `role`. Recorded as the daemon's.
    pub fn assign_on_entry(
        &self,
        id: &str,
        expected: u64,
        bot: &str,
        role: Role,
    ) -> anyhow::Result<Write<Item>> {
        versioned(self.conn, id, expected, |tx| {
            let before: Option<String> =
                tx.query_row("SELECT assignee FROM item WHERE id = ?1", [id], |r| {
                    r.get(0)
                })?;
            tx.execute(
                "UPDATE item SET assignee = ?2 WHERE id = ?1",
                params![id, bot],
            )?;
            record(
                tx,
                id,
                &Actor::Daemon,
                Event {
                    kind: ItemEventKind::Assigned,
                    from: before.as_deref(),
                    to: Some(bot),
                    field: Some(ON_ENTRY),
                    note: Some(role.as_str()),
                },
            )?;
            Ok(())
        })
    }

    /// The bots the card was assigned to, the latest first, each once.
    pub fn assignees_newest_first(&self, item_id: &str) -> anyhow::Result<Vec<String>> {
        let rows: Vec<String> = self
            .conn
            .prepare(
                r#"SELECT "to" FROM item_event
                   WHERE item_id = ?1 AND kind = 'assigned' AND "to" IS NOT NULL
                   ORDER BY at DESC, id DESC"#,
            )?
            .query_map([item_id], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        let mut out: Vec<String> = Vec::new();
        for bot in rows {
            if !out.contains(&bot) {
                out.push(bot);
            }
        }
        Ok(out)
    }

    /// The card waits in its column for `role`; replaces any earlier wait.
    pub fn set_role_wait(&self, project_id: &str, item: &Item, role: Role) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT INTO role_wait(item_id, project_id, board_id, column_key, role, since)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(item_id) DO UPDATE SET project_id = ?2, board_id = ?3,
                 column_key = ?4, role = ?5, since = ?6",
            params![
                item.id,
                project_id,
                item.board_id,
                item.column_key,
                role.as_str(),
                ts(now())
            ],
        )?;
        Ok(())
    }

    pub fn clear_role_wait(&self, item_id: &str) -> anyhow::Result<()> {
        self.conn
            .execute("DELETE FROM role_wait WHERE item_id = ?1", [item_id])?;
        Ok(())
    }

    /// The role whose on-entry assignment gave the card to its assignee,
    /// when that was the latest assignment.
    pub(super) fn on_entry_role(&self, item_id: &str) -> anyhow::Result<Option<Role>> {
        let row: Option<(Option<String>, Option<String>)> = self
            .conn
            .query_row(
                "SELECT field, note FROM item_event WHERE item_id = ?1 AND kind = 'assigned'
                 ORDER BY at DESC, id DESC LIMIT 1",
                [item_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        Ok(match row {
            Some((Some(field), Some(role))) if field == ON_ENTRY => Role::parse(&role),
            _ => None,
        })
    }
}

impl Db {
    /// A project's cards waiting for a role, oldest first.
    pub fn role_waits(&self, project_id: &str) -> anyhow::Result<Vec<RoleWait>> {
        let conn = self.lock();
        let rows: Vec<(String, String, String, String, String)> = conn
            .prepare(
                "SELECT item_id, board_id, column_key, role, since FROM role_wait
                 WHERE project_id = ?1 ORDER BY since, item_id",
            )?
            .query_map([project_id], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
            })?
            .collect::<Result<_, _>>()?;
        Ok(rows
            .into_iter()
            .filter_map(|(item_id, board_id, column_key, role, since)| {
                Some(RoleWait {
                    item_id,
                    board_id,
                    column_key,
                    role: Role::parse(&role)?,
                    since: super::board::parse_at(since),
                })
            })
            .collect())
    }
}
