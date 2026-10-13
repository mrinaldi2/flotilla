//! Column order and keys for `db::board_admin` (0.19.0 K4).

use rusqlite::params;

use super::board_admin::column;
use super::board_tx::BoardTx;
use crate::kanban::model::{Board, BoardColumn};

impl BoardTx<'_> {
    /// Moves `key` after `after` ("" = first), renumbering the board's columns.
    pub(super) fn reorder(
        &self,
        board: &Board,
        columns: &[BoardColumn],
        key: &str,
        after: &str,
    ) -> anyhow::Result<()> {
        let mut keys: Vec<&str> = columns
            .iter()
            .map(|c| c.key.as_str())
            .filter(|k| *k != key)
            .collect();
        let at = if after.is_empty() {
            0
        } else {
            column(columns, after)?;
            keys.iter()
                .position(|k| *k == after)
                .map_or(keys.len(), |i| i + 1)
        };
        keys.insert(at, key);
        let base = columns.iter().map(|c| c.ord).min().unwrap_or(0);
        for (i, k) in keys.iter().enumerate() {
            self.conn.execute(
                "UPDATE board_column SET ord = ?3 WHERE board_id = ?1 AND key = ?2",
                params![board.id, k, base + i as u32],
            )?;
        }
        Ok(())
    }

    /// A column key no column of the project uses, from the column's name.
    pub(super) fn free_column_key(&self, board: &Board, name: &str) -> anyhow::Result<String> {
        let slug: String = name
            .to_lowercase()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect::<String>()
            .split('-')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("-");
        let slug = if slug.is_empty() {
            "column".to_string()
        } else {
            slug
        };
        let taken = |k: &str| -> rusqlite::Result<bool> {
            self.conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM board_column WHERE project_id = ?1 AND key = ?2)",
                params![board.project_id, k],
                |r| r.get(0),
            )
        };
        let prefixed = format!("{}-{slug}", board.key.to_lowercase());
        for candidate in [slug.clone(), prefixed.clone()] {
            if !taken(&candidate)? {
                return Ok(candidate);
            }
        }
        let mut n = 2;
        loop {
            let candidate = format!("{prefixed}-{n}");
            if !taken(&candidate)? {
                return Ok(candidate);
            }
            n += 1;
        }
    }
}
