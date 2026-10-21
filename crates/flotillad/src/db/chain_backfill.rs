//! The default chain on boards made before 0.24.0 (`board_flow_v1`): once
//! per database, at startup, every custom board made from Simple + Review or
//! Software gets the chain its preset now starts with (`presets::chain`):
//! its Done column lets the Tester close a card, and Software's Ready lets
//! the Product move one there (`presets::chain_mover`). A board was made from
//! one of them when its columns' keys, without the board's prefix and but
//! for Cancelled, are that preset's. A column that already assigns a role
//! keeps it, and a board the owner changes afterwards keeps the change.

use std::collections::BTreeSet;

use rusqlite::{params, Connection, OptionalExtension};

use super::Db;
use crate::kanban::flow::Mover;
use crate::kanban::presets::{chain, chain_mover, Preset};

const DONE: &str = "board_chain_added";

impl Db {
    pub(super) fn backfill_board_chain(&self) -> anyhow::Result<()> {
        let mut conn = self.lock();
        let done: Option<String> = conn
            .query_row("SELECT value FROM meta WHERE key = ?1", [DONE], |r| {
                r.get(0)
            })
            .optional()?;
        if done.is_some() {
            return Ok(());
        }
        let tx = conn.transaction()?;
        let boards: Vec<(String, String)> = tx
            .prepare("SELECT id, key FROM board WHERE flow = 'custom' AND archived_at IS NULL")?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<_, _>>()?;
        for (board_id, board_key) in boards {
            add_chain(&tx, &board_id, &board_key)?;
        }
        tx.execute(
            "INSERT INTO meta(key, value) VALUES (?1, 'done')
             ON CONFLICT(key) DO UPDATE SET value = 'done'",
            [DONE],
        )?;
        tx.commit()?;
        Ok(())
    }
}

/// The preset keys a board's columns carry, `b2-review` read as `review`.
fn plain(key: &str, prefix: &str) -> String {
    key.strip_prefix(prefix).unwrap_or(key).to_string()
}

fn add_chain(tx: &Connection, board_id: &str, board_key: &str) -> anyhow::Result<()> {
    let prefix = format!("{}-", board_key.to_lowercase());
    let columns: Vec<(String, String, Option<String>, Option<String>)> = tx
        .prepare(
            "SELECT key, coalesce(kind, category), movers, on_entry FROM board_column
                  WHERE board_id = ?1",
        )?
        .query_map([board_id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        })?
        .collect::<Result<_, _>>()?;
    let keys: BTreeSet<String> = columns
        .iter()
        .filter(|(_, kind, ..)| kind != "cancelled")
        .map(|(key, ..)| plain(key, &prefix))
        .collect();
    let from_preset = [Preset::SimpleReview, Preset::Software]
        .iter()
        .any(|preset| {
            let wanted: BTreeSet<String> = preset
                .columns()
                .into_iter()
                .filter(|c| c.visible)
                .map(|c| c.key)
                .collect();
            wanted == keys
        });
    if !from_preset {
        return Ok(());
    }
    for (key, _, movers, on_entry) in &columns {
        if let (None, Some(role)) = (on_entry, chain(&plain(key, &prefix))) {
            tx.execute(
                "UPDATE board_column SET on_entry = ?3 WHERE board_id = ?1 AND key = ?2",
                params![board_id, key, role.as_str()],
            )?;
        }
        // No movers is any member, the role among them.
        let (Some(movers), Some(role)) = (movers.as_deref(), chain_mover(&plain(key, &prefix)))
        else {
            continue;
        };
        let mut movers: Vec<Mover> = serde_json::from_str(movers).unwrap_or_default();
        let mover = Mover::Roles(vec![role.as_str().into()]);
        if !movers.is_empty() && !movers.contains(&mover) {
            movers.push(mover);
            tx.execute(
                "UPDATE board_column SET movers = ?3 WHERE board_id = ?1 AND key = ?2",
                params![board_id, key, serde_json::to_string(&movers)?],
            )?;
        }
    }
    tx.execute(
        "UPDATE board SET version = version + 1 WHERE id = ?1",
        params![board_id],
    )?;
    Ok(())
}
