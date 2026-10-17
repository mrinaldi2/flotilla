//! The Research column on boards made before 0.21.0 (#43): once per
//! database, every board without one gets it just before the column work
//! starts in, Doing on a release-gated board. On a custom board it takes
//! that column's movers, the work that `work.started` moved there moves to
//! Research instead, and that column then wants the code research
//! recorded. A board whose owner later removes the column keeps it removed.

use std::collections::BTreeMap;

use rusqlite::{params, Connection, OptionalExtension};

use super::Db;
use crate::kanban::facts::Fact;
use crate::kanban::flow::EntryRule;

const DONE: &str = "research_columns_added";

struct Column {
    key: String,
    ord: u32,
    category: String,
    kind: String,
    movers: Option<String>,
    rules: String,
    allowed_from: String,
    visible: bool,
}

fn free_key(conn: &Connection, project_id: &str, board_key: &str) -> rusqlite::Result<String> {
    let taken = |k: &str| -> rusqlite::Result<bool> {
        conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM board_column WHERE project_id = ?1 AND key = ?2)",
            params![project_id, k],
            |r| r.get(0),
        )
    };
    let prefixed = format!("{}-research", board_key.to_lowercase());
    for candidate in ["research".to_string(), prefixed.clone()] {
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

fn columns(conn: &Connection, board_id: &str) -> rusqlite::Result<Vec<Column>> {
    conn.prepare(
        "SELECT key, ord, category, kind, movers, rules, allowed_from, visible FROM board_column
         WHERE board_id = ?1 ORDER BY ord",
    )?
    .query_map([board_id], |r| {
        Ok(Column {
            key: r.get(0)?,
            ord: r.get(1)?,
            category: r.get(2)?,
            kind: r.get::<_, Option<String>>(3)?.unwrap_or_default(),
            movers: r.get(4)?,
            rules: r.get(5)?,
            allowed_from: r.get(6)?,
            visible: r.get(7)?,
        })
    })?
    .collect()
}

impl Db {
    pub(super) fn backfill_research_columns(&self) -> anyhow::Result<()> {
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
        let boards: Vec<(String, String, String, String, String)> = tx
            .prepare(
                "SELECT id, project_id, key, flow, automations FROM board
                 WHERE archived_at IS NULL",
            )?
            .query_map([], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
            })?
            .collect::<Result<_, _>>()?;
        for (board_id, project_id, board_key, flow, automations) in boards {
            add_to_board(&tx, &board_id, &project_id, &board_key, &flow, &automations)?;
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

fn add_to_board(
    tx: &Connection,
    board_id: &str,
    project_id: &str,
    board_key: &str,
    flow: &str,
    automations: &str,
) -> anyhow::Result<()> {
    let cols = columns(tx, board_id)?;
    if cols.iter().any(|c| c.category == "research") {
        return Ok(());
    }
    let custom = flow == "custom";
    let mut automations: BTreeMap<String, String> =
        serde_json::from_str(automations).unwrap_or_default();
    let started = automations.get(Fact::WorkStarted.name()).cloned();
    let target = match custom {
        true => started
            .as_deref()
            .and_then(|key| cols.iter().find(|c| c.key == key && c.kind == "active"))
            .or_else(|| cols.iter().find(|c| c.visible && c.kind == "active")),
        false => cols.iter().find(|c| c.category == "doing"),
    };
    let Some(target) = target else {
        return Ok(());
    };
    let key = free_key(tx, project_id, board_key)?;
    tx.execute(
        "UPDATE board_column SET ord = ord + 1 WHERE board_id = ?1 AND ord >= ?2",
        params![board_id, target.ord],
    )?;
    let (movers, rules) = match custom {
        true => (
            target.movers.clone(),
            serde_json::to_string(&[EntryRule::AssigneeSet])?,
        ),
        false => (None, "[]".to_string()),
    };
    tx.execute(
        "INSERT INTO board_column(project_id, key, name, ord, category, wip_limit, wip_scope,
                                  visible, board_id, kind, movers, rules, allowed_from)
         VALUES (?1, ?2, 'Research', ?3, 'research', NULL, 'column', 1, ?4, 'active', ?5, ?6,
                 '[]')",
        params![project_id, key, target.ord, board_id, movers, rules],
    )?;
    if !custom {
        return Ok(());
    }
    let mut target_rules: Vec<EntryRule> = serde_json::from_str(&target.rules).unwrap_or_default();
    if !target_rules.contains(&EntryRule::ResearchRecorded) {
        target_rules.push(EntryRule::ResearchRecorded);
        tx.execute(
            "UPDATE board_column SET rules = ?3 WHERE project_id = ?1 AND key = ?2",
            params![
                project_id,
                target.key,
                serde_json::to_string(&target_rules)?
            ],
        )?;
    }
    // A column that only takes cards from some columns takes them from
    // Research too.
    let mut allowed: Vec<String> = serde_json::from_str(&target.allowed_from).unwrap_or_default();
    if !allowed.is_empty() {
        allowed.push(key.clone());
        tx.execute(
            "UPDATE board_column SET allowed_from = ?3 WHERE project_id = ?1 AND key = ?2",
            params![project_id, target.key, serde_json::to_string(&allowed)?],
        )?;
    }
    if started.as_deref() == Some(target.key.as_str()) {
        automations.insert(Fact::WorkStarted.name().to_string(), key);
    }
    tx.execute(
        "UPDATE board SET automations = ?2, version = version + 1 WHERE id = ?1",
        params![board_id, serde_json::to_string(&automations)?],
    )?;
    Ok(())
}
