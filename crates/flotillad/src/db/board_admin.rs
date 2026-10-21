//! A board's structure (0.19.0 K4): its name, columns, column rules and
//! automations, each change checked against the board `version` the caller
//! read. Who may make a change is `kanban::admin`'s call; this applies it.
//!
//! A release-gated board keeps its columns: it can be renamed, given label
//! groups, and its columns renamed, reordered, hidden or given a WIP limit,
//! nothing more.

use bus::contract::board as c;
use bus::now;
use rusqlite::params;

use super::board::to_text;
use super::board_tx::BoardTx;
use super::boards::board_in;
use super::{ts, Actor, MoveTo, Write};
use crate::decisions::{conflict, invalid, not_found};
use crate::kanban::facts::Fact;
use crate::kanban::flow::tokens;
use crate::kanban::labels;
use crate::kanban::model::{Board, BoardColumn, BoardFlow, ColumnKind, Role, StructurePolicy};
use crate::kanban::presets::category_of;

impl BoardTx<'_> {
    /// The board, if the caller read its current version.
    pub fn board_at(&self, board_id: &str, version: u64) -> anyhow::Result<Board> {
        let board = board_in(self.conn, board_id)?
            .ok_or_else(|| not_found(format!("no board {board_id}")))?;
        if board.version != version {
            return Err(conflict(format!(
                "the board {} changed since you read it (version {}, now {}); read it again",
                board.name, version, board.version
            )));
        }
        Ok(board)
    }

    fn bump_board(&self, board_id: &str) -> anyhow::Result<()> {
        self.conn.execute(
            "UPDATE board SET version = version + 1, updated_at = ?2 WHERE id = ?1",
            params![board_id, ts(now())],
        )?;
        Ok(())
    }

    fn board_columns(&self, board: &Board) -> anyhow::Result<Vec<BoardColumn>> {
        Ok(self
            .columns(&board.project_id)?
            .into_iter()
            .filter(|c| c.board_id == board.id)
            .collect())
    }

    pub fn update_board(&self, req: &c::BoardUpdate) -> anyhow::Result<()> {
        let board = self.board_at(&req.board_id, req.version)?;
        if let Some(name) = req.name.as_deref().map(str::trim) {
            anyhow::ensure!(!name.is_empty(), invalid("a board needs a name"));
            self.conn.execute(
                "UPDATE board SET name = ?2 WHERE id = ?1",
                params![board.id, name],
            )?;
        }
        if let Some(description) = &req.description {
            self.conn.execute(
                "UPDATE board SET description = ?2 WHERE id = ?1",
                params![board.id, description],
            )?;
        }
        if let Some(groups) = &req.label_groups {
            let groups = labels::checked(groups.values.iter().cloned().map(Into::into).collect())
                .map_err(invalid)?;
            self.conn.execute(
                "UPDATE board SET label_groups = ?2 WHERE id = ?1",
                params![board.id, serde_json::to_string(&groups)?],
            )?;
        }
        if let Some(policy) = &req.structure_policy {
            let policy = StructurePolicy::parse(policy)
                .ok_or_else(|| invalid(format!("“{policy}” is not a structure policy")))?;
            self.conn.execute(
                "UPDATE board SET structure_policy = ?2 WHERE id = ?1",
                params![board.id, to_text(&policy)],
            )?;
        }
        if req.is_default == Some(true) && !board.is_default {
            self.conn.execute(
                "UPDATE board SET is_default = 0 WHERE project_id = ?1",
                params![board.project_id],
            )?;
            self.conn.execute(
                "UPDATE board SET is_default = 1, archived_at = NULL WHERE id = ?1",
                params![board.id],
            )?;
        }
        if let Some(archived) = req.archived {
            anyhow::ensure!(
                !(archived && (board.is_default || req.is_default == Some(true))),
                invalid(
                    "the default board can't be archived; make another board the default first"
                )
            );
            self.conn.execute(
                "UPDATE board SET archived_at = ?2 WHERE id = ?1",
                params![board.id, archived.then(|| ts(now()))],
            )?;
        }
        self.bump_board(&board.id)
    }

    /// A new column; returns its key.
    pub fn add_column(&self, req: &c::ColumnAdd) -> anyhow::Result<String> {
        let board = self.board_at(&req.board_id, req.version)?;
        custom_only(&board, "add columns")?;
        let name = req.name.trim();
        anyhow::ensure!(!name.is_empty(), invalid("a column needs a name"));
        let kind =
            ColumnKind::from_wire(req.kind).map_err(|_| invalid("pick the column's kind"))?;
        let columns = self.board_columns(&board)?;
        let movers = tokens::movers(&req.movers).map_err(invalid)?;
        let rules = tokens::rules(&req.rules).map_err(invalid)?;
        sources_exist(&columns, &req.allowed_from)?;
        let on_entry = req.on_entry.map(on_entry_role).transpose()?.flatten();
        let key = self.free_column_key(&board, name)?;
        let ord = match req.after.as_deref() {
            Some(after) => column(&columns, after)?.ord + 1,
            None => columns.iter().map(|c| c.ord + 1).max().unwrap_or(0),
        };
        self.conn.execute(
            "UPDATE board_column SET ord = ord + 1 WHERE board_id = ?1 AND ord >= ?2",
            params![board.id, ord],
        )?;
        let wip_scope = req
            .wip_scope
            .map(crate::kanban::model::WipScope::from_wire)
            .transpose()?
            .unwrap_or(crate::kanban::model::WipScope::Column);
        self.conn.execute(
            "INSERT INTO board_column(project_id, key, name, ord, category, wip_limit, wip_scope,
                                      visible, board_id, kind, movers, rules, allowed_from,
                                      on_entry)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                board.project_id,
                key,
                name,
                ord,
                to_text(&category_of(kind)),
                req.wip_limit,
                to_text(&wip_scope),
                board.id,
                to_text(&kind),
                (!movers.is_empty())
                    .then(|| serde_json::to_string(&movers))
                    .transpose()?,
                serde_json::to_string(&rules)?,
                serde_json::to_string(&req.allowed_from)?,
                on_entry.map(Role::as_str)
            ],
        )?;
        self.bump_board(&board.id)?;
        Ok(key)
    }

    pub fn update_column(&self, req: &c::ColumnUpdate) -> anyhow::Result<()> {
        let board = self.board_at(&req.board_id, req.version)?;
        let columns = self.board_columns(&board)?;
        let col = column(&columns, &req.key)?;
        if let Some(name) = req.name.as_deref().map(str::trim) {
            anyhow::ensure!(!name.is_empty(), invalid("a column needs a name"));
            self.set_column(col, "name", name)?;
        }
        if let Some(kind) = req.kind {
            custom_only(&board, "change a column's kind")?;
            let kind = ColumnKind::from_wire(kind)?;
            self.set_column(col, "kind", to_text(&kind))?;
            self.set_column(col, "category", to_text(&category_of(kind)))?;
        }
        if req.clear_wip_limit == Some(true) {
            self.conn.execute(
                "UPDATE board_column SET wip_limit = NULL WHERE board_id = ?1 AND key = ?2",
                params![board.id, col.key],
            )?;
        } else if let Some(limit) = req.wip_limit {
            self.conn.execute(
                "UPDATE board_column SET wip_limit = ?3 WHERE board_id = ?1 AND key = ?2",
                params![board.id, col.key, limit],
            )?;
        }
        if let Some(scope) = req.wip_scope {
            let scope = crate::kanban::model::WipScope::from_wire(scope)?;
            self.set_column(col, "wip_scope", to_text(&scope))?;
        }
        if let Some(visible) = req.visible {
            self.conn.execute(
                "UPDATE board_column SET visible = ?3 WHERE board_id = ?1 AND key = ?2",
                params![board.id, col.key, visible],
            )?;
        }
        if req.set_policy == Some(true) {
            custom_only(&board, "change column rules")?;
            let movers = tokens::movers(&req.movers).map_err(invalid)?;
            let rules = tokens::rules(&req.rules).map_err(invalid)?;
            sources_exist(&columns, &req.allowed_from)?;
            self.conn.execute(
                "UPDATE board_column SET movers = ?3, rules = ?4, allowed_from = ?5
                 WHERE board_id = ?1 AND key = ?2",
                params![
                    board.id,
                    col.key,
                    (!movers.is_empty())
                        .then(|| serde_json::to_string(&movers))
                        .transpose()?,
                    serde_json::to_string(&rules)?,
                    serde_json::to_string(&req.allowed_from)?
                ],
            )?;
        }
        // Any board, release-gated too: the role is told after the move,
        // whichever rules allowed it (0.24.0).
        if let Some(role) = req.on_entry {
            self.conn.execute(
                "UPDATE board_column SET on_entry = ?3 WHERE board_id = ?1 AND key = ?2",
                params![board.id, col.key, on_entry_role(role)?.map(Role::as_str)],
            )?;
        }
        if let Some(after) = &req.after {
            self.reorder(&board, &columns, &col.key, after)?;
        }
        self.bump_board(&board.id)
    }

    /// Removes a column, moving its items to `move_items_to` first.
    pub fn remove_column(&self, req: &c::ColumnRemove, actor: &Actor<'_>) -> anyhow::Result<()> {
        let board = self.board_at(&req.board_id, req.version)?;
        custom_only(&board, "remove columns")?;
        let columns = self.board_columns(&board)?;
        let col = column(&columns, &req.key)?;
        let rest: Vec<&BoardColumn> = columns.iter().filter(|c| c.key != col.key).collect();
        anyhow::ensure!(
            rest.iter().any(|c| c.kind == ColumnKind::Open),
            invalid("a board needs an open column for new items")
        );
        // A draft or a discarded one only holds the column as a placeholder:
        // it moves to another open column with no history of a move.
        if let Some(open) = rest.iter().find(|c| c.kind == ColumnKind::Open) {
            self.conn.execute(
                "UPDATE item SET column_key = ?3, category = ?4
                 WHERE board_id = ?1 AND column_key = ?2 AND state != 'live'",
                params![board.id, col.key, open.key, open.category.as_str()],
            )?;
        }
        let items: Vec<(String, u64)> = self
            .conn
            .prepare(
                "SELECT id, version FROM item
                 WHERE board_id = ?1 AND column_key = ?2 AND state = 'live'",
            )?
            .query_map(params![board.id, col.key], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<_, _>>()?;
        if !items.is_empty() {
            let to = req.move_items_to.as_deref().ok_or_else(|| {
                invalid(format!(
                    "{} holds {} items; say where they go",
                    col.name,
                    items.len()
                ))
            })?;
            let to = column(&columns, to)?;
            anyhow::ensure!(
                to.key != col.key,
                invalid("move its items to another column")
            );
            let note = format!("column {} removed", col.name);
            for (id, version) in items {
                let to = MoveTo {
                    column: &to.key,
                    note: Some(&note),
                    wip_override: false,
                    first: false,
                };
                if let Write::Conflict(_) = self.move_item(&id, version, &to, actor)? {
                    return Err(conflict(format!("{id} changed while moving it")));
                }
            }
        }
        self.conn.execute(
            "DELETE FROM board_column WHERE board_id = ?1 AND key = ?2",
            params![board.id, col.key],
        )?;
        // Automations into it go; allowed sources forget it.
        let automations: std::collections::BTreeMap<String, String> = board
            .automations
            .into_iter()
            .filter(|(_, key)| *key != col.key)
            .collect();
        self.conn.execute(
            "UPDATE board SET automations = ?2 WHERE id = ?1",
            params![board.id, serde_json::to_string(&automations)?],
        )?;
        for other in rest {
            if other.policy.allowed_from.contains(&col.key) {
                let kept: Vec<&String> = other
                    .policy
                    .allowed_from
                    .iter()
                    .filter(|k| **k != col.key)
                    .collect();
                self.conn.execute(
                    "UPDATE board_column SET allowed_from = ?3 WHERE board_id = ?1 AND key = ?2",
                    params![board.id, other.key, serde_json::to_string(&kept)?],
                )?;
            }
        }
        self.bump_board(&board.id)
    }

    pub fn set_automation(&self, req: &c::AutomationSet) -> anyhow::Result<()> {
        let board = self.board_at(&req.board_id, req.version)?;
        custom_only(&board, "change automations")?;
        let fact = Fact::parse(&req.fact)
            .ok_or_else(|| invalid(format!("“{}” is not a fact", req.fact)))?;
        let mut automations = board.automations.clone();
        match req.column_key.as_deref().filter(|k| !k.is_empty()) {
            Some(key) => {
                column(&self.board_columns(&board)?, key)?;
                automations.insert(fact.name().to_string(), key.to_string());
            }
            None => {
                automations.remove(fact.name());
            }
        }
        self.conn.execute(
            "UPDATE board SET automations = ?2 WHERE id = ?1",
            params![board.id, serde_json::to_string(&automations)?],
        )?;
        self.bump_board(&board.id)
    }

    fn set_column(&self, col: &BoardColumn, field: &str, value: &str) -> anyhow::Result<()> {
        self.conn.execute(
            &format!("UPDATE board_column SET {field} = ?3 WHERE board_id = ?1 AND key = ?2"),
            params![col.board_id, col.key, value],
        )?;
        Ok(())
    }
}

fn custom_only(board: &Board, what: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        board.flow == BoardFlow::Custom,
        invalid(format!(
            "{} is release-gated and keeps its H-017 columns, so you can't {what}; \
             create a custom board for your own flow",
            board.name
        ))
    );
    Ok(())
}

pub(super) fn column<'c>(columns: &'c [BoardColumn], key: &str) -> anyhow::Result<&'c BoardColumn> {
    columns
        .iter()
        .find(|c| c.key == key)
        .ok_or_else(|| invalid(format!("this board has no column {key}")))
}

/// A column's `on_entry` from the wire: `ROLE_UNSPECIFIED` is none.
fn on_entry_role(wire: i32) -> anyhow::Result<Option<Role>> {
    if wire == c::Role::Unspecified as i32 {
        return Ok(None);
    }
    Role::from_wire(wire)
        .map(Some)
        .map_err(|_| invalid("pick a known role for the column's on-entry assignment"))
}

fn sources_exist(columns: &[BoardColumn], keys: &[String]) -> anyhow::Result<()> {
    for key in keys {
        column(columns, key)?;
    }
    Ok(())
}
