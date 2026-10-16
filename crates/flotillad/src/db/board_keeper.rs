//! Board storage for the keeper (0.19.0): the facts it reads about each live
//! card in one query, the cards a bot changed during a turn, and its marks
//! (`keeper_mark`), one per card and rule.

use chrono::{DateTime, Utc};
use rusqlite::{params, OptionalExtension, Row};

use super::board::{from_text, parse_at};
use super::{ts, Db};
use crate::kanban::model::ColumnCategory;

/// What the keeper reads about one live card.
#[derive(Debug, Clone, PartialEq)]
pub struct KeeperCard {
    pub id: String,
    pub board_id: String,
    pub title: String,
    pub column_key: String,
    pub category: ColumnCategory,
    pub assignee: Option<String>,
    pub entered_at: DateTime<Utc>,
    /// Its newest history event, by anyone.
    pub last_event_at: Option<DateTime<Utc>>,
    /// When it was last assigned to its current assignee.
    pub assigned_at: Option<DateTime<Utc>>,
    /// The current assignee's newest history event on it.
    pub assignee_acted_at: Option<DateTime<Utc>>,
}

/// One thing the keeper did, or waits to do, about a card.
#[derive(Debug, Clone, PartialEq)]
pub struct KeeperMark {
    pub item_id: String,
    pub rule: String,
    /// The card's assignee when it was written; empty for none.
    pub assignee: String,
    pub column_key: String,
    pub since: DateTime<Utc>,
    pub fired_at: Option<DateTime<Utc>>,
    pub detail: String,
}

/// A card its assignee changed during a turn.
#[derive(Debug, Clone, PartialEq)]
pub struct TurnWork {
    pub item_id: String,
    /// The newest change it made.
    pub last_at: DateTime<Utc>,
    /// Whether anyone moved the card during the turn.
    pub moved: bool,
}

fn at(r: &Row<'_>, i: usize) -> rusqlite::Result<Option<DateTime<Utc>>> {
    Ok(r.get::<_, Option<String>>(i)?.map(parse_at))
}

fn mark(r: &Row<'_>) -> rusqlite::Result<KeeperMark> {
    Ok(KeeperMark {
        item_id: r.get(0)?,
        rule: r.get(1)?,
        assignee: r.get(2)?,
        column_key: r.get(3)?,
        since: parse_at(r.get(4)?),
        fired_at: at(r, 5)?,
        detail: r.get(6)?,
    })
}

const MARK_COLUMNS: &str = "k.item_id, k.rule, k.assignee, k.column_key, k.since, k.fired_at, \
     k.detail";

/// The marks still true of their card: same column and, but for `stale`,
/// the same assignee. SQL over `k` (the mark) and `i` (its item).
pub(super) const MARK_HOLDS: &str = "k.column_key = i.column_key \
     AND (k.rule = 'stale' OR k.assignee = coalesce(i.assignee, ''))";

impl Db {
    /// The project's live cards, with what the keeper judges them by.
    pub fn keeper_cards(&self, project_id: &str) -> anyhow::Result<Vec<KeeperCard>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(
            r#"SELECT i.id, coalesce(i.board_id, ''), i.title, i.column_key, i.category,
                      i.assignee, i.state_entered_at,
                      (SELECT max(e.at) FROM item_event e WHERE e.item_id = i.id),
                      (SELECT max(e.at) FROM item_event e WHERE e.item_id = i.id
                          AND e.kind = 'assigned' AND e."to" = i.assignee),
                      (SELECT max(e.at) FROM item_event e WHERE e.item_id = i.id
                          AND e.actor = 'bot:' || i.assignee)
               FROM item i WHERE i.project_id = ?1 AND i.state = 'live'
               ORDER BY i.id"#,
        )?;
        let cards = stmt
            .query_map(params![project_id], |r| {
                Ok(KeeperCard {
                    id: r.get(0)?,
                    board_id: r.get(1)?,
                    title: r.get(2)?,
                    column_key: r.get(3)?,
                    category: from_text(r.get(4)?)?,
                    assignee: r.get(5)?,
                    entered_at: parse_at(r.get(6)?),
                    last_event_at: at(r, 7)?,
                    assigned_at: at(r, 8)?,
                    assignee_acted_at: at(r, 9)?,
                })
            })?
            .collect::<Result<_, _>>()?;
        Ok(cards)
    }

    /// Every mark on the project's cards, true or not, by card then rule.
    pub fn keeper_marks(&self, project_id: &str) -> anyhow::Result<Vec<KeeperMark>> {
        self.marks_where(project_id, "1")
    }

    /// The marks still true of their live card.
    pub fn keeper_marks_holding(&self, project_id: &str) -> anyhow::Result<Vec<KeeperMark>> {
        self.marks_where(project_id, &format!("i.state = 'live' AND {MARK_HOLDS}"))
    }

    fn marks_where(&self, project_id: &str, filter: &str) -> anyhow::Result<Vec<KeeperMark>> {
        let conn = self.lock();
        let sql = format!(
            "SELECT {MARK_COLUMNS} FROM keeper_mark k JOIN item i ON i.id = k.item_id
             WHERE i.project_id = ?1 AND {filter} ORDER BY k.item_id, k.rule"
        );
        let marks = conn
            .prepare(&sql)?
            .query_map(params![project_id], mark)?
            .collect::<Result<_, _>>()?;
        Ok(marks)
    }

    /// One mark, true or not.
    pub fn keeper_mark(&self, item_id: &str, rule: &str) -> anyhow::Result<Option<KeeperMark>> {
        let conn = self.lock();
        let sql = format!(
            "SELECT {MARK_COLUMNS} FROM keeper_mark k WHERE k.item_id = ?1 AND k.rule = ?2"
        );
        Ok(conn
            .query_row(&sql, params![item_id, rule], mark)
            .optional()?)
    }

    /// Write a mark, replacing the card's mark for the same rule.
    pub fn keeper_set(&self, m: &KeeperMark) -> anyhow::Result<()> {
        self.lock().execute(
            "INSERT INTO keeper_mark(item_id, rule, assignee, column_key, since, fired_at, detail)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(item_id, rule) DO UPDATE SET assignee = ?3, column_key = ?4,
                 since = ?5, fired_at = ?6, detail = ?7",
            params![
                m.item_id,
                m.rule,
                m.assignee,
                m.column_key,
                ts(m.since),
                m.fired_at.map(ts),
                m.detail
            ],
        )?;
        Ok(())
    }

    pub fn keeper_clear(&self, item_id: &str, rule: &str) -> anyhow::Result<()> {
        self.lock().execute(
            "DELETE FROM keeper_mark WHERE item_id = ?1 AND rule = ?2",
            params![item_id, rule],
        )?;
        Ok(())
    }

    /// Forget every mark of the project: the keeper is off for it.
    pub fn keeper_clear_project(&self, project_id: &str) -> anyhow::Result<usize> {
        Ok(self.lock().execute(
            "DELETE FROM keeper_mark WHERE item_id IN
                 (SELECT id FROM item WHERE project_id = ?1)",
            params![project_id],
        )?)
    }

    /// The live cards assigned to `bot_id` that it changed since `since`
    /// (an edit, a comment, a link, a block), newest first: a move or an
    /// assignment isn't work on the card.
    pub fn keeper_turn_work(
        &self,
        bot_id: &str,
        since: DateTime<Utc>,
    ) -> anyhow::Result<Vec<TurnWork>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(
            "SELECT e.item_id, max(e.at),
                    EXISTS(SELECT 1 FROM item_event m WHERE m.item_id = e.item_id
                           AND m.kind = 'moved' AND m.at >= ?2)
             FROM item_event e JOIN item i ON i.id = e.item_id
             WHERE e.actor = 'bot:' || ?1 AND e.at >= ?2 AND i.assignee = ?1
               AND i.state = 'live'
               AND e.kind IN ('edited', 'commented', 'linked', 'unlinked', 'blocked')
             GROUP BY e.item_id ORDER BY max(e.at) DESC, e.item_id",
        )?;
        let work = stmt
            .query_map(params![bot_id, ts(since)], |r| {
                Ok(TurnWork {
                    item_id: r.get(0)?,
                    last_at: parse_at(r.get(1)?),
                    moved: r.get(2)?,
                })
            })?
            .collect::<Result<_, _>>()?;
        Ok(work)
    }
}
