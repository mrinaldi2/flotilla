//! Board storage: attachment records (0.19.0 K4b). The bytes are in the
//! project's artifacts; this holds what an item carries. Attaching and
//! detaching are not versioned, as links aren't, and each records its
//! history event (and a revision while the item is a draft). A detached
//! attachment keeps its row, so a reference to it shows a placeholder.

use bus::{new_id, now};
use rusqlite::{params, Connection, OptionalExtension};

use super::board::{from_text, parse_at, to_text};
use super::board_drafts::revision;
use super::board_items::{record, Event};
use super::board_tx::BoardTx;
use super::{ts, Actor};
use crate::kanban::model::{AttachmentKind, ItemAttachment, ItemEventKind};

/// An attachment to record. Its bytes, if any, are already stored.
pub struct NewAttachment<'a> {
    pub item_id: &'a str,
    pub kind: AttachmentKind,
    pub name: &'a str,
    pub mime: &'a str,
    pub bytes: u64,
    pub sha256: &'a str,
    pub path: &'a str,
    pub url: &'a str,
    pub caption: &'a str,
}

const COLUMNS: &str = "id, item_id, kind, name, mime, bytes, sha256, path, url, caption, \
     added_by, at, removed_at IS NOT NULL";

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<ItemAttachment> {
    Ok(ItemAttachment {
        id: r.get(0)?,
        item_id: r.get(1)?,
        kind: from_text(r.get(2)?)?,
        name: r.get(3)?,
        mime: r.get(4)?,
        bytes: r.get(5)?,
        sha256: r.get(6)?,
        path: r.get(7)?,
        url: r.get(8)?,
        caption: r.get(9)?,
        added_by: r.get(10)?,
        at: parse_at(r.get(11)?),
        removed: r.get(12)?,
    })
}

fn attachment_in(conn: &Connection, id: &str) -> rusqlite::Result<Option<ItemAttachment>> {
    conn.query_row(
        &format!("SELECT {COLUMNS} FROM item_attachment WHERE id = ?1"),
        params![id],
        row,
    )
    .optional()
}

impl BoardTx<'_> {
    pub fn add_attachment(
        &self,
        new: &NewAttachment<'_>,
        actor: &Actor<'_>,
    ) -> anyhow::Result<ItemAttachment> {
        let id = new_id();
        self.conn.execute(
            "INSERT INTO item_attachment(id, item_id, kind, name, mime, bytes, sha256, path, url,
                                         caption, added_by, at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                id,
                new.item_id,
                to_text(&new.kind),
                new.name,
                new.mime,
                new.bytes,
                new.sha256,
                new.path,
                new.url,
                new.caption,
                actor.as_stored(),
                ts(now())
            ],
        )?;
        let event = Event {
            kind: ItemEventKind::Linked,
            from: None,
            to: Some(new.name),
            field: Some("attachment"),
            note: Some(&id),
        };
        record(self.conn, new.item_id, actor, event)?;
        revision(
            self.conn,
            new.item_id,
            actor,
            "attachment",
            None,
            Some(new.name),
        )?;
        Ok(attachment_in(self.conn, &id)?.expect("just inserted"))
    }

    /// Detach one; false when it isn't on the item or is already detached.
    pub fn detach(&self, item_id: &str, id: &str, actor: &Actor<'_>) -> anyhow::Result<bool> {
        let Some(found) = attachment_in(self.conn, id)?.filter(|a| a.item_id == item_id) else {
            return Ok(false);
        };
        let changed = self.conn.execute(
            "UPDATE item_attachment SET removed_at = ?2, removed_by = ?3
             WHERE id = ?1 AND removed_at IS NULL",
            params![id, ts(now()), actor.as_stored()],
        )?;
        if changed == 1 {
            let event = Event {
                kind: ItemEventKind::Unlinked,
                from: Some(&found.name),
                to: None,
                field: Some("attachment"),
                note: Some(id),
            };
            record(self.conn, item_id, actor, event)?;
            revision(
                self.conn,
                item_id,
                actor,
                "attachment",
                Some(&found.name),
                None,
            )?;
        }
        Ok(changed == 1)
    }

    /// An item's attachments, oldest first, detached ones included.
    pub fn attachments(&self, item_id: &str) -> anyhow::Result<Vec<ItemAttachment>> {
        let rows = self
            .conn
            .prepare(&format!(
                "SELECT {COLUMNS} FROM item_attachment WHERE item_id = ?1 ORDER BY at, id"
            ))?
            .query_map(params![item_id], row)?
            .collect::<Result<_, _>>()?;
        Ok(rows)
    }

    pub fn attachment(&self, id: &str) -> anyhow::Result<Option<ItemAttachment>> {
        Ok(attachment_in(self.conn, id)?)
    }

    /// The bytes an item's attached files hold, detached ones left out.
    pub fn attached_bytes(&self, item_id: &str) -> anyhow::Result<u64> {
        Ok(self.conn.query_row(
            "SELECT coalesce(sum(bytes), 0) FROM item_attachment
             WHERE item_id = ?1 AND removed_at IS NULL AND kind != 'doc'",
            params![item_id],
            |r| r.get(0),
        )?)
    }
}
