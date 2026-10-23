//! Board storage: the compact cards a board lists. A column's cards are the
//! live items only; a draft has a card of its own, for the owner's drafts
//! tray (0.19.0 K4b), and a discarded draft has none.

use bus::now;
use chrono::Duration;
use rusqlite::Connection;

use super::board::{from_json, from_text, parse_at, settings_in};
use crate::kanban::model::{ColumnCategory, ItemCard};

/// Active columns flag an item that has sat in them too long.
pub(crate) fn is_active(category: ColumnCategory) -> bool {
    matches!(
        category,
        ColumnCategory::Ready
            | ColumnCategory::Research
            | ColumnCategory::Doing
            | ColumnCategory::Review
            | ColumnCategory::Verify
    )
}

/// The live cards of a project matching `filter` (SQL over `i`, the item,
/// with `arg` as `?2`), in column then rank order.
pub(super) fn cards_in(
    conn: &Connection,
    project_id: &str,
    filter: &str,
    arg: Option<&str>,
) -> anyhow::Result<Vec<ItemCard>> {
    cards_with(conn, project_id, "i.state = 'live'", filter, arg)
}

/// The project's drafts, oldest first.
pub(super) fn draft_cards_in(conn: &Connection, project_id: &str) -> anyhow::Result<Vec<ItemCard>> {
    cards_with(conn, project_id, "i.state = 'draft'", "", None)
}

/// One item's card, live or draft; none for a discarded draft.
pub(super) fn card_in(
    conn: &Connection,
    project_id: &str,
    id: &str,
) -> anyhow::Result<Option<ItemCard>> {
    Ok(cards_with(
        conn,
        project_id,
        "i.state != 'discarded'",
        "AND i.id = ?2",
        Some(id),
    )?
    .into_iter()
    .next())
}

fn cards_with(
    conn: &Connection,
    project_id: &str,
    states: &str,
    filter: &str,
    arg: Option<&str>,
) -> anyhow::Result<Vec<ItemCard>> {
    let stale_after = settings_in(conn, project_id)?.map_or(24, |s| s.stale_after_hours);
    let stale_before = now() - Duration::hours(i64::from(stale_after));
    let sql = format!(
        "SELECT i.id, i.type, i.title, i.priority, i.size, i.rank, i.column_key, i.assignee,
                i.labels, i.blocked_since IS NOT NULL, i.category,
                i.state_entered_at, i.version,
                (SELECT count(*) FROM item_ac a WHERE a.item_id = i.id AND a.checked),
                (SELECT count(*) FROM item_ac a WHERE a.item_id = i.id),
                (SELECT max(at) FROM item_comment m WHERE m.item_id = i.id
                    AND (m.author = 'user' OR m.author LIKE 'device:%')),
                i.board_id, i.state = 'draft',
                CASE WHEN i.state = 'draft' THEN (
                    SELECT json_group_array(bot_id) FROM (
                        SELECT d.bot_id FROM item_draft_invite d WHERE d.item_id = i.id
                         ORDER BY d.at, d.bot_id))
                ELSE '[]' END,
                EXISTS(SELECT 1 FROM keeper_mark k WHERE k.item_id = i.id
                    AND k.rule IN ('stale', 'silent_stale') AND k.fired_at IS NOT NULL
                    AND {holds}),
                i.plan_node_id,
                (SELECT count(*) FROM question q JOIN decision d ON d.id = q.decision_id
                  WHERE q.item_id = i.id AND d.state IN ('open', 'held')),
                CASE WHEN i.category IN ('done', 'cancelled') THEN 0 ELSE (
                  SELECT count(*) FROM owner_question oq
                   WHERE oq.dismissed_at IS NULL AND oq.project_id = i.project_id
                     AND (oq.item_id = i.id
                          OR (oq.item_id IS NULL AND oq.title LIKE '%[card ' || i.id || ']%'))
                     AND (oq.message_num IS NULL OR NOT EXISTS (
                          SELECT 1 FROM message m JOIN conversation cv ON cv.id = m.conversation_id
                           WHERE cv.bot_id = oq.bot_id AND m.num > oq.message_num AND {from_owner}))
                     AND (oq.item_id IS NULL OR NOT EXISTS (
                          SELECT 1 FROM item_comment ic WHERE ic.item_id = i.id
                             AND ic.at > oq.created_at
                             AND (ic.author = 'user' OR ic.author LIKE 'device:%')))
                ) END
         FROM item i JOIN board_column c ON c.project_id = i.project_id AND c.key = i.column_key
         WHERE i.project_id = ?1 AND {states} {filter}
         ORDER BY c.ord, i.rank",
        holds = super::board_keeper::MARK_HOLDS,
        from_owner = super::owner_threads::FROM_OWNER
    );
    let mut stmt = conn.prepare(&sql)?;
    let map = |r: &rusqlite::Row<'_>| -> rusqlite::Result<ItemCard> {
        let category: ColumnCategory = from_text(r.get(10)?)?;
        let draft: bool = r.get(17)?;
        Ok(ItemCard {
            id: r.get(0)?,
            board_id: r.get::<_, Option<String>>(16)?.unwrap_or_default(),
            item_type: from_text(r.get(1)?)?,
            title: r.get(2)?,
            priority: from_text(r.get(3)?)?,
            size: r.get::<_, Option<String>>(4)?.map(from_text).transpose()?,
            rank: r.get(5)?,
            column_key: r.get(6)?,
            assignee: r.get(7)?,
            labels: from_json(r.get(8)?)?,
            blocked: r.get(9)?,
            // A draft is in no column, so it never goes stale. The keeper's
            // mark says so on its clock, and when its assignee went quiet.
            stale: !draft
                && (r.get::<_, bool>(19)?
                    || is_active(category) && parse_at(r.get(11)?) < stale_before),
            version: r.get(12)?,
            ac_checked: r.get(13)?,
            ac_total: r.get(14)?,
            owner_commented_at: r.get::<_, Option<String>>(15)?.map(parse_at),
            draft,
            invited: from_json(r.get(18)?)?,
            move_block: None,
            plan_node_id: r.get(20)?,
            questions_open: r.get(21)?,
            owner_waiting: r.get(22)?,
        })
    };
    let cards = match arg {
        Some(arg) => stmt
            .query_map(rusqlite::params![project_id, arg], map)?
            .collect::<Result<_, _>>()?,
        None => stmt
            .query_map(rusqlite::params![project_id], map)?
            .collect::<Result<_, _>>()?,
    };
    Ok(cards)
}
