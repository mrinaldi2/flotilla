//! Custom board storage (0.19.0 K3): the facts a board had no column for,
//! and creating a board from a preset.

use std::collections::BTreeMap;

use bus::now;
use rusqlite::params;

use super::board::to_text;
use super::board_tx::BoardTx;
use super::boards::{board_in, boards_in, free_key};
use super::{ts, Actor, Db};
use crate::kanban::flow::Mover;
use crate::kanban::model::Board;
use crate::kanban::presets::{Preset, Start};

impl BoardTx<'_> {
    pub fn board(&self, board_id: &str) -> anyhow::Result<Option<Board>> {
        Ok(board_in(self.conn, board_id)?)
    }

    /// Keep a fact the item's board had no column for, for its card.
    pub fn record_fact(
        &self,
        item_id: &str,
        fact: &str,
        note: Option<&str>,
        actor: &Actor<'_>,
    ) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT INTO item_fact(item_id, fact, note, actor, at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![item_id, fact, note, actor.as_stored(), ts(now())],
        )?;
        Ok(())
    }

    /// The facts kept for an item, oldest first.
    pub fn item_facts(&self, item_id: &str) -> anyhow::Result<Vec<String>> {
        Ok(self
            .conn
            .prepare("SELECT fact FROM item_fact WHERE item_id = ?1 ORDER BY at, id")?
            .query_map(params![item_id], |r| r.get(0))?
            .collect::<Result<_, _>>()?)
    }
}

/// A new board's columns on board `board_id`, from `first_ord` on, each key
/// passed through `rename`.
pub(super) fn insert_columns(
    tx: &rusqlite::Connection,
    project_id: &str,
    board_id: &str,
    start: &Start,
    first_ord: u32,
    rename: &dyn Fn(&String) -> String,
) -> anyhow::Result<()> {
    for ((i, c), category) in start.columns.iter().enumerate().zip(&start.categories) {
        let movers = (!c.policy.movers.is_empty())
            .then(|| serde_json::to_string::<Vec<Mover>>(&c.policy.movers))
            .transpose()?;
        let allowed: Vec<String> = c.policy.allowed_from.iter().map(rename).collect();
        tx.execute(
            "INSERT INTO board_column(project_id, key, name, ord, category, wip_limit,
                                      wip_scope, visible, board_id, kind, movers, rules,
                                      allowed_from)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                project_id,
                rename(&c.key),
                c.name,
                first_ord + i as u32,
                to_text(category),
                c.wip_limit,
                to_text(&c.wip_scope),
                c.visible,
                board_id,
                to_text(&c.kind),
                movers,
                serde_json::to_string(&c.policy.rules)?,
                serde_json::to_string(&allowed)?
            ],
        )?;
    }
    Ok(())
}

impl Db {
    /// A new board for a project that already has a board, from a preset.
    /// The first board of a project is made by `ensure_board_from`. Column keys
    /// stay unique in the project: a board's columns are prefixed with its
    /// key when another board uses the plain ones.
    pub fn create_board(
        &self,
        project_id: &str,
        name: &str,
        key: &str,
        preset: Preset,
    ) -> anyhow::Result<Board> {
        let mut conn = self.lock();
        let tx = conn.transaction()?;
        anyhow::ensure!(
            !boards_in(&tx, project_id)?.is_empty(),
            "this project has no board yet"
        );
        let key = free_key(&tx, key)?;
        let id = uuid::Uuid::new_v4().to_string();
        let at = ts(now());
        let columns = preset.columns();
        let taken = |k: &str| -> rusqlite::Result<bool> {
            tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM board_column WHERE project_id = ?1 AND key = ?2)",
                params![project_id, k],
                |r| r.get(0),
            )
        };
        let mut keys = BTreeMap::new();
        for c in &columns {
            let plain = c.key.to_string();
            let mapped = if taken(&plain)? {
                format!("{}-{plain}", key.to_lowercase())
            } else {
                plain.clone()
            };
            keys.insert(plain, mapped);
        }
        let rename = |k: &String| keys.get(k).cloned().unwrap_or_else(|| k.clone());
        let automations: BTreeMap<String, String> = preset
            .automations()
            .iter()
            .map(|(fact, col)| (fact.clone(), rename(col)))
            .collect();
        let ord: u32 = tx.query_row(
            "SELECT coalesce(max(ord), 0) + 1 FROM board WHERE project_id = ?1",
            params![project_id],
            |r| r.get(0),
        )?;
        tx.execute(
            "INSERT INTO board(id, project_id, name, key, ord, flow, automations, created_at,
                               updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
            params![
                id,
                project_id,
                name,
                key,
                ord,
                to_text(&preset.flow()),
                serde_json::to_string(&automations)?,
                at
            ],
        )?;
        let next_ord: u32 = tx.query_row(
            "SELECT coalesce(max(ord), 0) + 1 FROM board_column WHERE project_id = ?1",
            params![project_id],
            |r| r.get(0),
        )?;
        insert_columns(&tx, project_id, &id, &Start::of(preset), next_ord, &rename)?;
        let board = board_in(&tx, &id)?.expect("just inserted");
        tx.commit()?;
        Ok(board)
    }
}
