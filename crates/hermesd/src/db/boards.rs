//! The boards of a project (0.19.0 K2): the `board` table. A project that
//! has a board has a default one; its columns, items and templates name it.

use rusqlite::{params, Connection, OptionalExtension};

use super::board::from_text;
use super::{parse_ts, Db};
use crate::kanban::model::Board;

const BOARD_COLUMNS: &str =
    "id, project_id, name, key, description, ord, is_default, structure_policy, archived_at, version";

fn board_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Board> {
    Ok(Board {
        id: r.get(0)?,
        project_id: r.get(1)?,
        name: r.get(2)?,
        key: r.get(3)?,
        description: r.get(4)?,
        ord: r.get(5)?,
        is_default: r.get(6)?,
        structure_policy: from_text(r.get(7)?)?,
        archived_at: r.get::<_, Option<String>>(8)?.map(|t| parse_ts(&t)),
        version: r.get(9)?,
    })
}

/// The id a project's first board gets, the one the K2 migration gave every
/// existing board.
pub(super) fn default_board_id(project_id: &str) -> String {
    format!("{project_id}/main")
}

/// A project's boards, the default first, then by `ord`.
pub(super) fn boards_in(conn: &Connection, project_id: &str) -> rusqlite::Result<Vec<Board>> {
    conn.prepare(&format!(
        "SELECT {BOARD_COLUMNS} FROM board WHERE project_id = ?1
         ORDER BY is_default DESC, ord, created_at"
    ))?
    .query_map(params![project_id], board_from_row)?
    .collect()
}

/// A project's default board, if it has a board at all.
pub(super) fn default_board_in(
    conn: &Connection,
    project_id: &str,
) -> rusqlite::Result<Option<Board>> {
    conn.query_row(
        &format!("SELECT {BOARD_COLUMNS} FROM board WHERE project_id = ?1 AND is_default = 1"),
        params![project_id],
        board_from_row,
    )
    .optional()
}

/// Whether any board, or any project's board settings, uses `key`.
pub(super) fn key_taken(conn: &Connection, key: &str) -> rusqlite::Result<bool> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM board WHERE key = ?1)
             OR EXISTS(SELECT 1 FROM board_settings WHERE key = ?1)",
        params![key],
        |r| r.get(0),
    )
}

/// A key no other board uses: the preferred one, else it with a number.
pub(super) fn free_key(conn: &Connection, preferred: &str) -> rusqlite::Result<String> {
    if !key_taken(conn, preferred)? {
        return Ok(preferred.to_string());
    }
    let mut n = 2;
    loop {
        let candidate = format!("{preferred}{n}");
        if !key_taken(conn, &candidate)? {
            return Ok(candidate);
        }
        n += 1;
    }
}

impl Db {
    pub fn boards(&self, project_id: &str) -> anyhow::Result<Vec<Board>> {
        Ok(boards_in(&self.lock(), project_id)?)
    }

    pub fn default_board(&self, project_id: &str) -> anyhow::Result<Option<Board>> {
        Ok(default_board_in(&self.lock(), project_id)?)
    }
}
