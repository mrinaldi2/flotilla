//! Board storage: drafts (0.19.0 K4b). A draft has its id and a placeholder
//! column, and no column lists it. The owner invites bots to discuss it;
//! every change to it is a revision with its author, and the owner's "last
//! looked" is the newest revision seen. Finalizing puts it live in a column;
//! discarding keeps it, and its history, out of every list.

use bus::now;
use rusqlite::{params, Connection, OptionalExtension};

use super::board::{from_text, parse_at, to_text};
use super::board_edit::versioned;
use super::board_items::{record, Event, Write};
use super::board_ranks::rank_last;
use super::board_tx::BoardTx;
use super::{ts, Actor};
use crate::kanban::model::{
    BoardColumn, ColumnCategory, DraftRevision, Item, ItemEventKind, ItemState,
};

/// A draft with bot changes the owner hasn't seen: the Needs you row.
#[derive(Debug, Clone, PartialEq)]
pub struct UnseenDraft {
    pub item_id: String,
    pub title: String,
    /// The bot that made the latest of them, as stored (`bot:<id>`).
    pub actor: String,
    pub at: chrono::DateTime<chrono::Utc>,
}

/// Record a revision of a draft: `field` went from `from` to `to`.
pub(super) fn revision(
    conn: &Connection,
    item_id: &str,
    actor: &Actor<'_>,
    field: &str,
    from: Option<&str>,
    to: Option<&str>,
) -> rusqlite::Result<()> {
    conn.execute(
        r#"INSERT INTO item_draft_revision(item_id, actor, at, field, "from", "to")
           SELECT id, ?2, ?3, ?4, ?5, ?6 FROM item WHERE id = ?1 AND state = 'draft'"#,
        params![item_id, actor.as_stored(), ts(now()), field, from, to],
    )?;
    Ok(())
}

fn revision_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<DraftRevision> {
    Ok(DraftRevision {
        id: r.get(0)?,
        item_id: r.get(1)?,
        actor: r.get(2)?,
        at: parse_at(r.get(3)?),
        field: r.get(4)?,
        from: r.get(5)?,
        to: r.get(6)?,
    })
}

/// After an edit of what was a draft, a revision per field that changed.
pub(super) fn revised(
    conn: &Connection,
    before: Option<&Item>,
    write: &Write<Item>,
    actor: &Actor<'_>,
) -> anyhow::Result<()> {
    let (Some(before), Write::Done(after)) = (before, write) else {
        return Ok(());
    };
    if before.state != ItemState::Draft {
        return Ok(());
    }
    let text = |item: &Item| -> Vec<(&'static str, String)> {
        let ac: Vec<&str> = item
            .acceptance_criteria
            .iter()
            .map(|a| a.text.as_str())
            .collect();
        vec![
            ("title", item.title.clone()),
            ("description", item.description.clone()),
            ("type", item.item_type.as_str().to_string()),
            (
                "size",
                item.size.map(|s| s.as_str()).unwrap_or("").to_string(),
            ),
            ("priority", item.priority.as_str().to_string()),
            ("labels", item.labels.join(", ")),
            ("acceptance_criteria", ac.join("\n")),
            (
                "platforms",
                item.platforms
                    .iter()
                    .map(|p| p.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
            ),
            ("parent", item.parent_id.clone().unwrap_or_default()),
        ]
    };
    for ((field, from), (_, to)) in text(before).into_iter().zip(text(after)) {
        if from != to {
            revision(conn, &before.id, actor, field, Some(&from), Some(&to))?;
        }
    }
    Ok(())
}

impl BoardTx<'_> {
    /// Invite a bot to discuss a draft; false when it already was.
    pub fn invite(&self, item_id: &str, bot_id: &str, actor: &Actor<'_>) -> anyhow::Result<bool> {
        let added = self.conn.execute(
            "INSERT OR IGNORE INTO item_draft_invite(item_id, bot_id, invited_by, at)
             VALUES (?1, ?2, ?3, ?4)",
            params![item_id, bot_id, actor.as_stored(), ts(now())],
        )?;
        if added == 1 {
            let event = Event {
                kind: ItemEventKind::Edited,
                from: None,
                to: Some(bot_id),
                field: Some("invited"),
                note: None,
            };
            record(self.conn, item_id, actor, event)?;
        }
        Ok(added == 1)
    }

    /// Take an invitation back; false when there was none.
    pub fn uninvite(&self, item_id: &str, bot_id: &str, actor: &Actor<'_>) -> anyhow::Result<bool> {
        let removed = self.conn.execute(
            "DELETE FROM item_draft_invite WHERE item_id = ?1 AND bot_id = ?2",
            params![item_id, bot_id],
        )?;
        if removed == 1 {
            let event = Event {
                kind: ItemEventKind::Edited,
                from: Some(bot_id),
                to: None,
                field: Some("invited"),
                note: None,
            };
            record(self.conn, item_id, actor, event)?;
        }
        Ok(removed == 1)
    }

    /// The bots invited to discuss a draft, by id, in invitation order.
    pub fn invited(&self, item_id: &str) -> anyhow::Result<Vec<String>> {
        let rows = self
            .conn
            .prepare("SELECT bot_id FROM item_draft_invite WHERE item_id = ?1 ORDER BY at, bot_id")?
            .query_map(params![item_id], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        Ok(rows)
    }

    /// A draft's revisions after `since`, oldest first.
    pub fn draft_revisions(
        &self,
        item_id: &str,
        since: Option<i64>,
    ) -> anyhow::Result<Vec<DraftRevision>> {
        let rows = self
            .conn
            .prepare(
                r#"SELECT id, item_id, actor, at, field, "from", "to" FROM item_draft_revision
                   WHERE item_id = ?1 AND id > ?2 ORDER BY id"#,
            )?
            .query_map(params![item_id, since.unwrap_or(0)], revision_row)?
            .collect::<Result<_, _>>()?;
        Ok(rows)
    }

    /// One revision of the draft `item_id`, by its id.
    pub fn draft_revision(&self, item_id: &str, id: i64) -> anyhow::Result<Option<DraftRevision>> {
        Ok(self
            .conn
            .query_row(
                r#"SELECT id, item_id, actor, at, field, "from", "to" FROM item_draft_revision
                   WHERE item_id = ?1 AND id = ?2"#,
                params![item_id, id],
                revision_row,
            )
            .optional()?)
    }

    /// The newest revision the owner has seen; 0 for none.
    pub fn draft_seen(&self, item_id: &str) -> anyhow::Result<i64> {
        Ok(self
            .conn
            .query_row(
                "SELECT rev FROM item_draft_seen WHERE item_id = ?1",
                params![item_id],
                |r| r.get(0),
            )
            .optional()?
            .unwrap_or(0))
    }

    /// The owner has seen revisions up to `rev`. Never moves back.
    pub fn set_draft_seen(&self, item_id: &str, rev: i64) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT INTO item_draft_seen(item_id, rev) VALUES (?1, ?2)
             ON CONFLICT(item_id) DO UPDATE SET rev = max(rev, excluded.rev)",
            params![item_id, rev],
        )?;
        Ok(())
    }

    /// Mark every revision of a draft seen.
    pub fn see_all(&self, item_id: &str) -> anyhow::Result<()> {
        let newest: i64 = self.conn.query_row(
            "SELECT coalesce(max(id), 0) FROM item_draft_revision WHERE item_id = ?1",
            params![item_id],
            |r| r.get(0),
        )?;
        self.set_draft_seen(item_id, newest)
    }

    /// The project's drafts a bot changed since the owner last looked.
    pub fn unseen_drafts(&self, project_id: &str) -> anyhow::Result<Vec<UnseenDraft>> {
        let rows = self
            .conn
            .prepare(
                "SELECT i.id, i.title, r.actor, r.at FROM item i
                 JOIN item_draft_revision r ON r.id = (
                     SELECT max(id) FROM item_draft_revision
                     WHERE item_id = i.id AND actor LIKE 'bot:%')
                 WHERE i.project_id = ?1 AND i.state = 'draft'
                   AND r.id > coalesce((SELECT rev FROM item_draft_seen s
                                        WHERE s.item_id = i.id), 0)
                 ORDER BY r.at",
            )?
            .query_map(params![project_id], |r| {
                Ok(UnseenDraft {
                    item_id: r.get(0)?,
                    title: r.get(1)?,
                    actor: r.get(2)?,
                    at: parse_at(r.get(3)?),
                })
            })?
            .collect::<Result<_, _>>()?;
        Ok(rows)
    }

    /// Put a draft live in `column`, last in its rank order. The move from
    /// no column is its first entry, so cycle time starts here.
    pub fn go_live(
        &self,
        id: &str,
        expected: u64,
        column: &BoardColumn,
        actor: &Actor<'_>,
    ) -> anyhow::Result<Write<Item>> {
        versioned(self.conn, id, expected, |tx| {
            let project_id: String = tx.query_row(
                "SELECT project_id FROM item WHERE id = ?1",
                params![id],
                |r| r.get(0),
            )?;
            let rank = rank_last(tx, &project_id, &column.key)?;
            let at = ts(now());
            let done_at = (column.category == ColumnCategory::Done).then_some(at.clone());
            tx.execute(
                "UPDATE item SET state = 'live', column_key = ?2, category = ?3, rank = ?4,
                                 state_entered_at = ?5, done_at = ?6
                 WHERE id = ?1 AND state = 'draft'",
                params![id, column.key, to_text(&column.category), rank, at, done_at],
            )?;
            state_event(tx, id, actor, "live", None)?;
            let moved = Event {
                kind: ItemEventKind::Moved,
                from: None,
                to: Some(&column.key),
                field: None,
                note: Some("finalized from a draft"),
            };
            record(tx, id, actor, moved)?;
            Ok(())
        })
    }

    /// Drop a draft. It keeps its row and history, out of every list.
    pub fn discard(
        &self,
        id: &str,
        expected: u64,
        reason: &str,
        actor: &Actor<'_>,
    ) -> anyhow::Result<Write<Item>> {
        versioned(self.conn, id, expected, |tx| {
            tx.execute(
                "UPDATE item SET state = 'discarded' WHERE id = ?1 AND state = 'draft'",
                params![id],
            )?;
            state_event(tx, id, actor, "discarded", Some(reason))
        })
    }

    /// The item's state, or `None` when there is no such item.
    pub fn item_state(&self, id: &str) -> anyhow::Result<Option<ItemState>> {
        Ok(self
            .conn
            .query_row("SELECT state FROM item WHERE id = ?1", params![id], |r| {
                from_text(r.get(0)?)
            })
            .optional()?)
    }
}

fn state_event(
    tx: &Connection,
    id: &str,
    actor: &Actor<'_>,
    to: &str,
    note: Option<&str>,
) -> anyhow::Result<()> {
    let event = Event {
        kind: ItemEventKind::Edited,
        from: Some("draft"),
        to: Some(to),
        field: Some("state"),
        note,
    };
    record(tx, id, actor, event)?;
    Ok(())
}
