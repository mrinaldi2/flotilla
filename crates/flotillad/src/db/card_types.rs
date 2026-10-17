//! What the 0.21 card types migration leaves for the daemon (#43, step 2):
//! the new item-type templates for projects that already have a board, and
//! the cards it removed (`item_purged`), whose files go and whose assignees
//! hear of it once.

use rusqlite::{params, Connection, OpenFlags, OptionalExtension};

use super::Db;
use crate::kanban::defaults;
use crate::kanban::model::ItemType;

/// A card the migration removed, or (in a dry run) would remove.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetiredCard {
    pub id: String,
    pub project_id: String,
    pub item_type: String,
    pub title: String,
    pub assignee: Option<String>,
    pub column_key: String,
}

fn retired(r: &rusqlite::Row<'_>) -> rusqlite::Result<RetiredCard> {
    Ok(RetiredCard {
        id: r.get(0)?,
        project_id: r.get(1)?,
        item_type: r.get(2)?,
        title: r.get(3)?,
        assignee: r.get(4)?,
        column_key: r.get(5)?,
    })
}

/// What upgrading the database at `path` removes, read without migrating
/// it: the epic and feature cards still there (before the migration), or the
/// ones it recorded (after), and how many chores became or become tasks.
pub fn dry_run(path: &std::path::Path) -> anyhow::Result<(Vec<RetiredCard>, u64)> {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let has = |table: &str| -> rusqlite::Result<bool> {
        conn.query_row(
            "SELECT count(*) > 0 FROM sqlite_master WHERE type = 'table' AND name = ?1",
            [table],
            |r| r.get(0),
        )
    };
    if has("item_purged")? {
        let cards = conn
            .prepare(
                "SELECT id, project_id, type, title, assignee, column_key FROM item_purged
                 ORDER BY project_id, id",
            )?
            .query_map([], retired)?
            .collect::<Result<_, _>>()?;
        return Ok((cards, 0));
    }
    if !has("item")? {
        return Ok((Vec::new(), 0));
    }
    let cards = conn
        .prepare(
            "SELECT id, project_id, type, title, assignee, column_key FROM item
             WHERE type IN ('epic', 'feature') ORDER BY project_id, seq",
        )?
        .query_map([], retired)?
        .collect::<Result<_, _>>()?;
    let chores = conn.query_row("SELECT count(*) FROM item WHERE type = 'chore'", [], |r| {
        r.get(0)
    })?;
    Ok((cards, chores))
}

impl Db {
    /// Gives every project with a board the task and design templates, and
    /// its bug template a "Code research" section, once. A task takes the
    /// project's feature template's Definition of Ready (its label and spec
    /// rules included); then the epic, feature and chore templates go.
    pub(super) fn backfill_card_type_templates(&self) -> anyhow::Result<()> {
        let conn = self.lock();
        let projects: Vec<String> = conn
            .prepare("SELECT project_id FROM board_settings")?
            .query_map([], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        let at = super::ts(chrono::Utc::now());
        let latest = |project_id: &str, name: &str| -> rusqlite::Result<Option<(u32, String)>> {
            conn.query_row(
                "SELECT version, body FROM template
                 WHERE project_id = ?1 AND kind = 'item_type' AND name = ?2 AND board_id IS NULL
                 ORDER BY version DESC LIMIT 1",
                params![project_id, name],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
        };
        for project_id in projects {
            let feature_ready = latest(&project_id, "feature")?
                .and_then(|(_, body)| serde_json::from_str::<serde_json::Value>(&body).ok())
                .and_then(|body| body.get("ready").cloned());
            for (item_type, mut body) in defaults::item_templates() {
                let name = super::board::to_text(&item_type);
                let (version, next) = match latest(&project_id, name)? {
                    None => {
                        if let (ItemType::Task, Some(ready)) = (item_type, &feature_ready) {
                            body["ready"] = ready.clone();
                        }
                        (1, body.to_string())
                    }
                    Some((version, old)) => match with_code_research(&old) {
                        Some(next) => (version + 1, next),
                        None => continue,
                    },
                };
                conn.execute(
                    "INSERT INTO template(project_id, kind, name, version, body, created_at)
                     VALUES (?1, 'item_type', ?2, ?3, ?4, ?5)",
                    params![project_id, name, version, next, at],
                )?;
            }
        }
        conn.execute(
            "DELETE FROM template WHERE kind = 'item_type' AND name IN ('epic', 'feature', 'chore')",
            [],
        )?;
        Ok(())
    }

    /// The removed cards whose files and assignees haven't been seen to.
    pub fn retired_cards_pending(&self) -> anyhow::Result<Vec<RetiredCard>> {
        let conn = self.lock();
        let cards = conn
            .prepare(
                "SELECT id, project_id, type, title, assignee, column_key FROM item_purged
                 WHERE handled_at IS NULL ORDER BY project_id, id",
            )?
            .query_map([], retired)?
            .collect::<Result<_, _>>()?;
        Ok(cards)
    }

    pub fn retired_card_handled(&self, id: &str) -> anyhow::Result<()> {
        self.lock().execute(
            "UPDATE item_purged SET handled_at = ?2 WHERE id = ?1",
            params![id, super::ts(chrono::Utc::now())],
        )?;
        Ok(())
    }
}

/// A bug template (or a task's) without the "Code research" section, with
/// it added before the last section; `None` when it has it, or isn't a bug's
/// or a task's shape.
fn with_code_research(body: &str) -> Option<String> {
    let mut value: serde_json::Value = serde_json::from_str(body).ok()?;
    let sections = value.get_mut("sections")?.as_array_mut()?;
    let heading = crate::kanban::guards::RESEARCH_SECTION;
    let is_bug = sections.iter().any(|s| s == "Steps to reproduce");
    if !is_bug || sections.iter().any(|s| s == heading) {
        return None;
    }
    sections.push(heading.into());
    Some(value.to_string())
}
