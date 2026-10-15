//! Where a card lands in a column's order: after the last card, or before
//! the first.

use rusqlite::{params, Connection};

use crate::kanban::rank;

/// The rank after the last card in a column.
pub(super) fn rank_last(
    conn: &Connection,
    project_id: &str,
    column_key: &str,
) -> anyhow::Result<String> {
    let last: Option<String> = conn.query_row(
        "SELECT max(rank) FROM item WHERE project_id = ?1 AND column_key = ?2",
        params![project_id, column_key],
        |r| r.get(0),
    )?;
    rank::between(last.as_deref(), None).ok_or_else(|| anyhow::anyhow!("no rank after {last:?}"))
}

/// The rank before the first card in a column.
pub(super) fn rank_first(
    conn: &Connection,
    project_id: &str,
    column_key: &str,
) -> anyhow::Result<String> {
    let first: Option<String> = conn.query_row(
        "SELECT min(rank) FROM item WHERE project_id = ?1 AND column_key = ?2",
        params![project_id, column_key],
        |r| r.get(0),
    )?;
    rank::between(None, first.as_deref()).ok_or_else(|| anyhow::anyhow!("no rank before {first:?}"))
}
