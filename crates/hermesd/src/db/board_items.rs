//! Board storage, part 2: items. Creating one takes the next id from the
//! project's sequence, ranks it last in the inbox and records a `created`
//! event in the same transaction. Reads assemble the contract's `Item` and
//! `ItemCard`.

use crate::kanban::model::{
    AcceptanceCriterion, Blocked, ColumnCategory, Item, ItemCard, ItemEvent, ItemEventKind,
    ItemPerson, ItemType, ItemVerification, Platform, Priority, Size,
};
use bus::now;
use rusqlite::{params, Connection, OptionalExtension};

use super::board::{
    from_json, from_json_list, from_text, item_template, parse_at, settings_in, to_json_list,
    to_text,
};
use super::board_ranks::rank_last;
use super::board_tx::BoardTx;
use super::boards::{board_in, default_board_in};
use super::{ts, Actor, Db};
use crate::kanban::defaults;

/// The outcome of a versioned write: done, or refused because someone else
/// changed the record first, with what it is now.
#[derive(Debug)]
pub enum Write<T> {
    Done(T),
    Conflict(Box<T>),
}

pub struct NewItem<'a> {
    pub project_id: &'a str,
    /// The board it goes on; `None` is the project's default board.
    pub board_id: Option<&'a str>,
    pub item_type: ItemType,
    pub title: &'a str,
    /// Empty means the type's template sections.
    pub description: &'a str,
    pub platforms: &'a [Platform],
    pub size: Option<Size>,
    pub priority: Priority,
    pub labels: &'a [String],
    pub parent_id: Option<&'a str>,
    pub acceptance_criteria: &'a [String],
}

/// One history entry, written inside the caller's transaction.
pub(super) struct Event<'a> {
    pub kind: ItemEventKind,
    pub from: Option<&'a str>,
    pub to: Option<&'a str>,
    pub field: Option<&'a str>,
    pub note: Option<&'a str>,
}

pub(super) fn record(
    conn: &Connection,
    item_id: &str,
    actor: &Actor<'_>,
    e: Event<'_>,
) -> rusqlite::Result<()> {
    conn.execute(
        r#"INSERT INTO item_event(item_id, project_id, at, actor, kind, "from", "to", field, note)
           SELECT id, project_id, ?2, ?3, ?4, ?5, ?6, ?7, ?8 FROM item WHERE id = ?1"#,
        params![
            item_id,
            ts(now()),
            actor.as_stored(),
            to_text(&e.kind),
            e.from,
            e.to,
            e.field,
            e.note
        ],
    )?;
    Ok(())
}

const ITEM_COLUMNS: &str = "id, seq, type, title, description, platforms, size, priority, rank, \
     column_key, category, blocked_by, blocked_reason, blocked_since, assignee, parent_id, \
     release_id, labels, created_by, created_at, updated_at, state_entered_at, done_at, version, \
     board_id, state";

pub(super) fn item_in(conn: &Connection, id: &str) -> rusqlite::Result<Option<Item>> {
    let Some(mut item) = conn
        .query_row(
            &format!("SELECT {ITEM_COLUMNS} FROM item WHERE id = ?1"),
            params![id],
            |r| {
                let blocked_reason: Option<String> = r.get(12)?;
                let blocked_since: Option<String> = r.get(13)?;
                Ok(Item {
                    id: r.get(0)?,
                    board_id: r.get::<_, Option<String>>(24)?.unwrap_or_default(),
                    seq: r.get(1)?,
                    item_type: from_text(r.get(2)?)?,
                    title: r.get(3)?,
                    description: r.get(4)?,
                    platforms: from_json_list(r.get(5)?)?,
                    size: r.get::<_, Option<String>>(6)?.map(from_text).transpose()?,
                    priority: from_text(r.get(7)?)?,
                    rank: r.get(8)?,
                    column_key: r.get(9)?,
                    category: from_text(r.get(10)?)?,
                    blocked: match (blocked_reason, blocked_since) {
                        (Some(reason), Some(since)) => Some(Blocked {
                            by: r.get(11)?,
                            reason,
                            since: parse_at(since),
                        }),
                        _ => None,
                    },
                    assignee: r.get(14)?,
                    parent_id: r.get(15)?,
                    release_id: r.get(16)?,
                    labels: from_json(r.get(17)?)?,
                    created_by: r.get(18)?,
                    created_at: parse_at(r.get(19)?),
                    updated_at: parse_at(r.get(20)?),
                    state_entered_at: parse_at(r.get(21)?),
                    done_at: r.get::<_, Option<String>>(22)?.map(parse_at),
                    version: r.get(23)?,
                    state: from_text(r.get(25)?)?,
                    acceptance_criteria: Vec::new(),
                    people: Vec::new(),
                    verifications: Vec::new(),
                })
            },
        )
        .optional()?
    else {
        return Ok(None);
    };
    item.acceptance_criteria = conn
        .prepare(
            "SELECT a.idx, a.text, a.checked, a.checked_by, a.checked_at, a.machine,
                    p.item_id IS NOT NULL
             FROM item_ac a
             LEFT JOIN item_ac_post_install p ON p.item_id = a.item_id AND p.text = a.text
             WHERE a.item_id = ?1 ORDER BY a.idx",
        )?
        .query_map(params![id], |r| {
            Ok(AcceptanceCriterion {
                idx: r.get(0)?,
                text: r.get(1)?,
                checked: r.get(2)?,
                checked_by: r.get(3)?,
                checked_at: r.get::<_, Option<String>>(4)?.map(parse_at),
                machine: r.get(5)?,
                post_install: r.get(6)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    item.people = conn
        .prepare("SELECT bot_id, role FROM item_person WHERE item_id = ?1 ORDER BY role, bot_id")?
        .query_map(params![id], |r| {
            Ok(ItemPerson {
                bot_id: r.get(0)?,
                role: from_text(r.get(1)?)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    item.verifications = conn
        .prepare(
            "SELECT machine, result, by, at, note FROM item_verification
             WHERE item_id = ?1 ORDER BY machine",
        )?
        .query_map(params![id], |r| {
            Ok(ItemVerification {
                machine: r.get(0)?,
                result: from_text(r.get(1)?)?,
                by: r.get(2)?,
                at: parse_at(r.get(3)?),
                note: r.get(4)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(Some(item))
}

impl BoardTx<'_> {
    pub fn create_item(&self, new: &NewItem<'_>, actor: &Actor<'_>) -> anyhow::Result<Item> {
        self.insert_item(new, actor, false)
    }

    /// A draft (0.19.0 K4b): it has its id, and the board's first open
    /// column as a placeholder, but no column lists it until it is
    /// finalized. Its `created` event names no column, so flow metrics
    /// start at the finalize.
    pub fn create_draft(&self, new: &NewItem<'_>, actor: &Actor<'_>) -> anyhow::Result<Item> {
        self.insert_item(new, actor, true)
    }

    fn insert_item(
        &self,
        new: &NewItem<'_>,
        actor: &Actor<'_>,
        draft: bool,
    ) -> anyhow::Result<Item> {
        let tx = self.conn;
        let settings = settings_in(tx, new.project_id)?
            .ok_or_else(|| anyhow::anyhow!("this project has no board"))?;
        let board = match new.board_id {
            Some(id) => board_in(tx, id)?.filter(|b| b.project_id == new.project_id),
            None => default_board_in(tx, new.project_id)?,
        }
        .ok_or_else(|| anyhow::anyhow!("this project has no such board"))?;
        let inbox: String = tx.query_row(
            "SELECT key FROM board_column
             WHERE project_id = ?1 AND board_id = ?2 AND category = 'inbox'
             ORDER BY ord LIMIT 1",
            params![new.project_id, board.id],
            |r| r.get(0),
        )?;
        // One sequence per project; the board's key is the prefix.
        let seq = settings.next_seq;
        let id = format!("{}-{seq:03}", board.key);
        tx.execute(
            "UPDATE board_settings SET next_seq = next_seq + 1 WHERE project_id = ?1",
            params![new.project_id],
        )?;
        let item_type = to_text(&new.item_type);
        let description = if new.description.trim().is_empty() {
            item_template(tx, new.project_id, item_type)?
                .map(|t| defaults::description_skeleton(&t))
                .unwrap_or_default()
        } else {
            new.description.to_string()
        };
        let at = ts(now());
        let rank = rank_last(tx, new.project_id, &inbox)?;
        tx.execute(
            &format!(
                "INSERT INTO item(id, project_id, seq, type, title, description, platforms, size,
                                  priority, rank, column_key, category, labels, parent_id,
                                  created_by, created_at, updated_at, state_entered_at, board_id,
                                  state)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, '{}', ?12, ?13, ?14, ?15, ?15, ?15,
                         ?16, ?17)",
                to_text(&ColumnCategory::Inbox)
            ),
            params![
                id,
                new.project_id,
                seq,
                item_type,
                new.title,
                description,
                to_json_list(new.platforms),
                new.size.map(|s| to_text(&s)),
                to_text(&new.priority),
                rank,
                inbox,
                serde_json::to_string(new.labels)?,
                new.parent_id,
                actor.as_stored(),
                at,
                board.id,
                if draft { "draft" } else { "live" }
            ],
        )?;
        for (idx, text) in new.acceptance_criteria.iter().enumerate() {
            tx.execute(
                "INSERT INTO item_ac(item_id, idx, text) VALUES (?1, ?2, ?3)",
                params![id, idx as u32, text],
            )?;
        }
        record(
            tx,
            &id,
            actor,
            Event {
                kind: ItemEventKind::Created,
                from: None,
                to: (!draft).then_some(inbox.as_str()),
                field: None,
                note: None,
            },
        )?;
        let item = item_in(tx, &id)?.expect("just inserted");
        Ok(item)
    }
}

impl Db {
    pub fn get_item(&self, id: &str) -> anyhow::Result<Option<Item>> {
        Ok(item_in(&self.lock(), id)?)
    }

    /// Every card on a project's board, in column then rank order.
    pub fn board_cards(&self, project_id: &str) -> anyhow::Result<Vec<ItemCard>> {
        self.cards_where(project_id, "", None)
    }

    /// Cards whose title, description or comments match an FTS5 query.
    pub fn search_items(&self, project_id: &str, query: &str) -> anyhow::Result<Vec<ItemCard>> {
        self.cards_where(
            project_id,
            "AND i.id IN (SELECT item_id FROM item_fts WHERE item_fts MATCH ?2)",
            Some(query),
        )
    }

    fn cards_where(
        &self,
        project_id: &str,
        filter: &str,
        arg: Option<&str>,
    ) -> anyhow::Result<Vec<ItemCard>> {
        super::board_cards::cards_in(&self.lock(), project_id, filter, arg)
    }

    /// An item's history, oldest first.
    pub fn item_events(&self, item_id: &str) -> anyhow::Result<Vec<ItemEvent>> {
        let conn = self.lock();
        let rows = conn
            .prepare(
                r#"SELECT id, item_id, at, actor, kind, "from", "to", field, note
                   FROM item_event WHERE item_id = ?1 ORDER BY id"#,
            )?
            .query_map(params![item_id], |r| {
                Ok(ItemEvent {
                    id: r.get(0)?,
                    item_id: r.get(1)?,
                    at: parse_at(r.get(2)?),
                    actor: r.get(3)?,
                    kind: from_text(r.get(4)?)?,
                    from: r.get(5)?,
                    to: r.get(6)?,
                    field: r.get(7)?,
                    note: r.get(8)?,
                })
            })?
            .collect::<Result<_, _>>()?;
        Ok(rows)
    }
}
