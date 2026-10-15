//! Board storage, part 3: changing items. Every write names the version it
//! read and records its history event in the same transaction; a stale version
//! returns `Write::Conflict` with the item as it is now. Moves here are raw:
//! the guards that decide whether a move is allowed belong to the service
//! layer above (B3), which calls these once they pass.

use crate::kanban::model::{ColumnCategory, Item, ItemEventKind, ItemType, Priority, Size};
use bus::now;
use rusqlite::{params, Connection, OptionalExtension};

use super::board::{from_text, to_text};
use super::board_items::{item_in, record, Event, Write};
use super::board_notes::replace_ac;
use super::board_ranks::{rank_first, rank_last};
use super::board_tx::BoardTx;
use super::{ts, Actor};
use crate::kanban::guards::WIP_OVERRIDE_LABEL;
use crate::kanban::rank;

/// Fields an edit may change; `None` leaves a field as it is.
#[derive(Default)]
pub struct ItemEdit<'a> {
    pub title: Option<&'a str>,
    pub description: Option<&'a str>,
    pub size: Option<Option<Size>>,
    pub priority: Option<Priority>,
    pub labels: Option<&'a [String]>,
    /// One label group's values, replacing that group's labels (in `labels`
    /// when it is given, else in the item's).
    pub group: Option<(&'a str, &'a [String])>,
    pub parent_id: Option<Option<&'a str>>,
    pub item_type: Option<ItemType>,
    /// The whole list; a criterion whose text stays keeps its check.
    pub acceptance_criteria: Option<&'a [String]>,
}

/// Where a move puts an item.
#[derive(Clone, Copy, Default)]
pub struct MoveTo<'a> {
    pub column: &'a str,
    /// What the history event says.
    pub note: Option<&'a str>,
    /// Flags the card for as long as it sits in the column it went over in.
    pub wip_override: bool,
    /// The top of the column rather than the bottom (returned work first).
    pub first: bool,
}

/// Take the item's next version, or say why not.
pub(super) fn claim(tx: &Connection, id: &str, expected: u64) -> anyhow::Result<Option<Item>> {
    let claimed = tx.execute(
        "UPDATE item SET version = version + 1, updated_at = ?3 WHERE id = ?1 AND version = ?2",
        params![id, expected, ts(now())],
    )?;
    if claimed == 1 {
        return Ok(None);
    }
    let current = item_in(tx, id)?.ok_or_else(|| anyhow::anyhow!("item {id} not found"))?;
    Ok(Some(current))
}

/// Run a versioned change inside the caller's transaction: claim the
/// version, apply, then return the item. A conflict writes nothing.
pub(super) fn versioned(
    tx: &Connection,
    id: &str,
    expected: u64,
    apply: impl FnOnce(&Connection) -> anyhow::Result<()>,
) -> anyhow::Result<Write<Item>> {
    if let Some(current) = claim(tx, id, expected)? {
        return Ok(Write::Conflict(Box::new(current)));
    }
    apply(tx)?;
    Ok(Write::Done(item_in(tx, id)?.expect("claimed above")))
}

fn edited(
    tx: &Connection,
    id: &str,
    actor: &Actor<'_>,
    field: &str,
    from: &str,
    to: &str,
) -> rusqlite::Result<()> {
    record(
        tx,
        id,
        actor,
        Event {
            kind: ItemEventKind::Edited,
            from: Some(from),
            to: Some(to),
            field: Some(field),
            note: None,
        },
    )
}

impl BoardTx<'_> {
    pub fn update_item(
        &self,
        id: &str,
        expected: u64,
        edit: &ItemEdit<'_>,
        actor: &Actor<'_>,
    ) -> anyhow::Result<Write<Item>> {
        let draft = item_in(self.conn, id)?;
        let write = versioned(self.conn, id, expected, |tx| {
            let before = item_in(tx, id)?.expect("claimed");
            if let Some(title) = edit.title.filter(|t| *t != before.title) {
                tx.execute(
                    "UPDATE item SET title = ?2 WHERE id = ?1",
                    params![id, title],
                )?;
                edited(tx, id, actor, "title", &before.title, title)?;
            }
            if let Some(description) = edit.description.filter(|d| *d != before.description) {
                tx.execute(
                    "UPDATE item SET description = ?2 WHERE id = ?1",
                    params![id, description],
                )?;
                // The text itself is in the item; history says that it changed.
                edited(tx, id, actor, "description", "", "")?;
            }
            if let Some(size) = edit.size.filter(|s| *s != before.size) {
                let text = |s: Option<Size>| s.map(|s| to_text(&s)).unwrap_or_default();
                tx.execute(
                    "UPDATE item SET size = ?2 WHERE id = ?1",
                    params![id, size.map(|s| to_text(&s))],
                )?;
                edited(tx, id, actor, "size", text(before.size), text(size))?;
            }
            if let Some(priority) = edit.priority.filter(|p| *p != before.priority) {
                tx.execute(
                    "UPDATE item SET priority = ?2 WHERE id = ?1",
                    params![id, to_text(&priority)],
                )?;
                edited(
                    tx,
                    id,
                    actor,
                    "priority",
                    to_text(&before.priority),
                    to_text(&priority),
                )?;
            }
            if let Some(item_type) = edit.item_type.filter(|t| *t != before.item_type) {
                tx.execute(
                    "UPDATE item SET type = ?2 WHERE id = ?1",
                    params![id, item_type.as_str()],
                )?;
                edited(
                    tx,
                    id,
                    actor,
                    "type",
                    before.item_type.as_str(),
                    item_type.as_str(),
                )?;
            }
            let labels = match (edit.labels, edit.group) {
                (labels, Some((group, values))) => Some(crate::kanban::labels::replace_group(
                    labels.unwrap_or(&before.labels),
                    group,
                    values,
                )),
                (labels, None) => labels.map(<[String]>::to_vec),
            };
            if let Some(labels) = labels.filter(|l| *l != before.labels) {
                let (from, to) = (
                    serde_json::to_string(&before.labels)?,
                    serde_json::to_string(&labels)?,
                );
                tx.execute("UPDATE item SET labels = ?2 WHERE id = ?1", params![id, to])?;
                edited(tx, id, actor, "labels", &from, &to)?;
            }
            if let Some(texts) = edit.acceptance_criteria {
                replace_ac(tx, &before, texts, actor)?;
            }
            if let Some(parent) = edit.parent_id.filter(|p| *p != before.parent_id.as_deref()) {
                tx.execute(
                    "UPDATE item SET parent_id = ?2 WHERE id = ?1",
                    params![id, parent],
                )?;
                edited(
                    tx,
                    id,
                    actor,
                    "parent",
                    before.parent_id.as_deref().unwrap_or(""),
                    parent.unwrap_or(""),
                )?;
            }
            Ok(())
        })?;
        super::board_drafts::revised(self.conn, draft.as_ref(), &write, actor)?;
        Ok(write)
    }

    /// Put an item into another column, last in its rank order. Entering a
    /// done column stamps `done_at`; leaving one clears it.
    pub fn move_item(
        &self,
        id: &str,
        expected: u64,
        to: &MoveTo<'_>,
        actor: &Actor<'_>,
    ) -> anyhow::Result<Write<Item>> {
        versioned(self.conn, id, expected, |tx| move_in(tx, id, to, actor))
    }

    /// Place an item between two neighbours in its column (either may be
    /// absent: the top or the bottom).
    pub fn rank_item(
        &self,
        id: &str,
        expected: u64,
        after_id: Option<&str>,
        before_id: Option<&str>,
        actor: &Actor<'_>,
    ) -> anyhow::Result<Write<Item>> {
        versioned(self.conn, id, expected, |tx| {
            let rank_of = |other: Option<&str>| -> anyhow::Result<Option<String>> {
                other
                    .map(|o| {
                        tx.query_row("SELECT rank FROM item WHERE id = ?1", params![o], |r| {
                            r.get(0)
                        })
                        .optional()?
                        .ok_or_else(|| anyhow::anyhow!("item {o} not found"))
                    })
                    .transpose()
            };
            let (low, high) = (rank_of(after_id)?, rank_of(before_id)?);
            let old: String =
                tx.query_row("SELECT rank FROM item WHERE id = ?1", params![id], |r| {
                    r.get(0)
                })?;
            let new = rank::between(low.as_deref(), high.as_deref())
                .ok_or_else(|| anyhow::anyhow!("those neighbours are not in order"))?;
            tx.execute("UPDATE item SET rank = ?2 WHERE id = ?1", params![id, new])?;
            record(
                tx,
                id,
                actor,
                Event {
                    kind: ItemEventKind::Ranked,
                    from: Some(&old),
                    to: Some(&new),
                    field: None,
                    note: None,
                },
            )?;
            Ok(())
        })
    }

    pub fn assign_item(
        &self,
        id: &str,
        expected: u64,
        assignee: Option<&str>,
        actor: &Actor<'_>,
    ) -> anyhow::Result<Write<Item>> {
        versioned(self.conn, id, expected, |tx| {
            let before = item_in(tx, id)?.expect("claimed");
            tx.execute(
                "UPDATE item SET assignee = ?2 WHERE id = ?1",
                params![id, assignee],
            )?;
            record(
                tx,
                id,
                actor,
                Event {
                    kind: ItemEventKind::Assigned,
                    from: before.assignee.as_deref(),
                    to: assignee,
                    field: None,
                    note: None,
                },
            )?;
            Ok(())
        })
    }
}

/// The write half of a move, inside the caller's transaction; the caller has
/// already claimed the item's version.
pub(super) fn move_in(
    tx: &Connection,
    id: &str,
    to: &MoveTo<'_>,
    actor: &Actor<'_>,
) -> anyhow::Result<()> {
    let MoveTo {
        column: to_column,
        note,
        wip_override,
        first,
    } = *to;
    let before = item_in(tx, id)?.expect("claimed");
    let project_id: String = tx.query_row(
        "SELECT project_id FROM item WHERE id = ?1",
        params![id],
        |r| r.get(0),
    )?;
    let category: ColumnCategory = tx
        .query_row(
            "SELECT category FROM board_column WHERE project_id = ?1 AND key = ?2",
            params![project_id, to_column],
            |r| from_text(r.get(0)?),
        )
        .optional()?
        .ok_or_else(|| anyhow::anyhow!("no column {to_column}"))?;
    let rank = if first {
        rank_first(tx, &project_id, to_column)?
    } else {
        rank_last(tx, &project_id, to_column)?
    };
    let at = ts(now());
    let done_at = matches!(category, ColumnCategory::Done).then_some(at.clone());
    tx.execute(
        "UPDATE item SET column_key = ?2, category = ?3, rank = ?4, state_entered_at = ?5,
                             done_at = ?6 WHERE id = ?1",
        params![id, to_column, to_text(&category), rank, at, done_at],
    )?;
    let mut labels = before.labels.clone();
    labels.retain(|l| l != WIP_OVERRIDE_LABEL);
    if wip_override {
        labels.push(WIP_OVERRIDE_LABEL.to_string());
    }
    if labels != before.labels {
        tx.execute(
            "UPDATE item SET labels = ?2 WHERE id = ?1",
            params![id, serde_json::to_string(&labels)?],
        )?;
    }
    record(
        tx,
        id,
        actor,
        Event {
            kind: ItemEventKind::Moved,
            from: Some(&before.column_key),
            to: Some(to_column),
            field: None,
            note,
        },
    )?;
    Ok(())
}
