//! Board storage, part 1: a project's board settings, columns, roles and
//! templates, seeded together when the board is enabled. It stores and
//! returns the board's own types (`kanban::model`); adapters map them to the
//! wire contract through `kanban::contract`.

use std::collections::BTreeMap;

use crate::kanban::model::{
    BoardColumn, BoardSettings, ProjectRole, Role, Template, TemplateKind, TextValue,
};
use bus::now;
use rusqlite::{params, Connection, OptionalExtension};
use serde::de::DeserializeOwned;
use serde_json::Value;

use super::boards::{default_board_id, free_key};
use super::{parse_ts, ts, Db};
use crate::kanban::defaults;

/// A value-set member as stored.
pub(super) fn to_text<T: TextValue>(value: &T) -> &'static str {
    value.as_text()
}

fn bad_text(text: &str) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        0,
        rusqlite::types::Type::Text,
        format!("unknown stored value {text:?}").into(),
    )
}

/// The inverse of [`to_text`].
pub(super) fn from_text<T: TextValue>(text: String) -> rusqlite::Result<T> {
    T::from_text(&text).ok_or_else(|| bad_text(&text))
}

fn machines_to_json(machines: &BTreeMap<String, Vec<String>>) -> String {
    serde_json::to_string(machines).expect("strings serialise")
}

pub(super) fn from_json<T: DeserializeOwned>(text: String) -> rusqlite::Result<T> {
    serde_json::from_str(&text).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
    })
}

const SETTINGS_COLUMNS: &str = "project_id, key, next_seq, stale_after_hours, \
     required_machines, home_daemon_id, version";

fn settings_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<BoardSettings> {
    Ok(BoardSettings {
        project_id: r.get(0)?,
        key: r.get(1)?,
        next_seq: r.get(2)?,
        stale_after_hours: r.get(3)?,
        required_machines: from_json(r.get(4)?)?,
        home_daemon_id: r.get(5)?,
        version: r.get(6)?,
    })
}

pub(super) fn settings_in(
    conn: &Connection,
    project_id: &str,
) -> rusqlite::Result<Option<BoardSettings>> {
    conn.query_row(
        &format!("SELECT {SETTINGS_COLUMNS} FROM board_settings WHERE project_id = ?1"),
        params![project_id],
        settings_from_row,
    )
    .optional()
}

impl Db {
    pub fn board_settings(&self, project_id: &str) -> anyhow::Result<Option<BoardSettings>> {
        Ok(settings_in(&self.lock(), project_id)?)
    }

    /// Enable the board for a project, or return the existing one unchanged.
    /// A new board gets the default columns, the item-type templates and the
    /// project's lead as its lead.
    pub fn ensure_board(
        &self,
        project_id: &str,
        home_daemon_id: &str,
        key: Option<&str>,
    ) -> anyhow::Result<BoardSettings> {
        let mut conn = self.lock();
        if let Some(existing) = settings_in(&conn, project_id)? {
            return Ok(existing);
        }
        let (project_name, lead_bot_id): (String, Option<String>) = conn
            .query_row(
                "SELECT name, lead_bot_id FROM project WHERE id = ?1 AND deleted_at IS NULL",
                params![project_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?
            .ok_or_else(|| anyhow::anyhow!("project not found"))?;
        let tx = conn.transaction()?;
        let key = free_key(
            &tx,
            &key.map_or_else(|| defaults::key_for(&project_name), str::to_string),
        )?;
        let at = ts(now());
        tx.execute(
            "INSERT INTO board_settings(project_id, key, home_daemon_id, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?4)",
            params![project_id, key, home_daemon_id, at],
        )?;
        let board_id = default_board_id(project_id);
        tx.execute(
            "INSERT INTO board(id, project_id, name, key, is_default, created_at, updated_at)
             VALUES (?1, ?2, 'Main', ?3, 1, ?4, ?4)",
            params![board_id, project_id, key, at],
        )?;
        for (ord, c) in defaults::COLUMNS.iter().enumerate() {
            tx.execute(
                "INSERT INTO board_column(project_id, key, name, ord, category, wip_limit,
                                          wip_scope, visible, board_id, kind)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    project_id,
                    c.key,
                    c.name,
                    ord as u32,
                    to_text(&c.category),
                    c.wip_limit,
                    to_text(&c.wip_scope),
                    c.visible,
                    board_id,
                    to_text(&c.category.kind())
                ],
            )?;
        }
        for (item_type, body) in defaults::item_templates() {
            tx.execute(
                "INSERT INTO template(project_id, kind, name, version, body, created_at)
                 VALUES (?1, 'item_type', ?2, 1, ?3, ?4)",
                params![project_id, to_text(&item_type), body.to_string(), at],
            )?;
        }
        // The project's lead leads the board; every other role is given
        // explicitly, by the owner or the lead (`role_set`).
        tx.execute(
            "INSERT OR IGNORE INTO project_role(project_id, role, bot_id)
             SELECT ?1, ?2, id FROM bot
             WHERE id = ?3 AND project_id = ?1 AND deleted_at IS NULL",
            params![project_id, to_text(&Role::Lead), lead_bot_id],
        )?;
        let settings = settings_in(&tx, project_id)?.expect("just inserted");
        tx.commit()?;
        Ok(settings)
    }

    pub fn board_columns(&self, project_id: &str) -> anyhow::Result<Vec<BoardColumn>> {
        Ok(columns_in(&self.lock(), project_id)?)
    }

    pub fn project_roles(&self, project_id: &str) -> anyhow::Result<Vec<ProjectRole>> {
        Ok(roles_in(&self.lock(), project_id)?)
    }

    /// Give a bot a role (or update a tester's machine).
    pub fn set_project_role(&self, role: &ProjectRole) -> anyhow::Result<()> {
        self.lock().execute(
            "INSERT INTO project_role(project_id, role, bot_id, machine) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(project_id, role, bot_id) DO UPDATE SET machine = excluded.machine",
            params![
                role.project_id,
                to_text(&role.role),
                role.bot_id,
                role.machine
            ],
        )?;
        Ok(())
    }

    pub fn remove_project_role(
        &self,
        project_id: &str,
        role: Role,
        bot_id: &str,
    ) -> anyhow::Result<bool> {
        let removed = self.lock().execute(
            "DELETE FROM project_role WHERE project_id = ?1 AND role = ?2 AND bot_id = ?3",
            params![project_id, to_text(&role), bot_id],
        )?;
        Ok(removed > 0)
    }

    /// Sets or clears a column's WIP limit. False when there is no such column.
    pub fn set_column_limit(
        &self,
        project_id: &str,
        key: &str,
        limit: Option<u32>,
    ) -> anyhow::Result<bool> {
        let changed = self.lock().execute(
            "UPDATE board_column SET wip_limit = ?3 WHERE project_id = ?1 AND key = ?2",
            params![project_id, key, limit],
        )?;
        Ok(changed == 1)
    }

    /// The latest version of every template of one kind.
    pub fn templates(&self, project_id: &str, kind: TemplateKind) -> anyhow::Result<Vec<Template>> {
        let conn = self.lock();
        let rows = conn
            .prepare(
                "SELECT t.project_id, t.kind, t.name, t.version, t.body FROM template t
                 WHERE t.project_id = ?1 AND t.kind = ?2
                   AND t.version = (SELECT max(version) FROM template
                                    WHERE project_id = t.project_id AND kind = t.kind
                                      AND name = t.name)
                 ORDER BY t.name",
            )?
            .query_map(params![project_id, to_text(&kind)], |r| {
                Ok(Template {
                    project_id: r.get(0)?,
                    kind: from_text(r.get(1)?)?,
                    name: r.get(2)?,
                    version: r.get(3)?,
                    body: from_json(r.get(4)?)?,
                })
            })?
            .collect::<Result<_, _>>()?;
        Ok(rows)
    }

    /// Settings fields a client may change; `None` leaves one as it is.
    pub fn update_board_settings(
        &self,
        project_id: &str,
        expected_version: u64,
        stale_after_hours: Option<u32>,
        required_machines: Option<&BTreeMap<String, Vec<String>>>,
    ) -> anyhow::Result<super::board_items::Write<BoardSettings>> {
        let conn = self.lock();
        let changed = conn.execute(
            "UPDATE board_settings
             SET stale_after_hours = coalesce(?3, stale_after_hours),
                 required_machines = coalesce(?4, required_machines),
                 version = version + 1, updated_at = ?5
             WHERE project_id = ?1 AND version = ?2",
            params![
                project_id,
                expected_version,
                stale_after_hours,
                required_machines.map(machines_to_json),
                ts(now())
            ],
        )?;
        let current = settings_in(&conn, project_id)?
            .ok_or_else(|| anyhow::anyhow!("this project has no board"))?;
        Ok(if changed == 1 {
            super::board_items::Write::Done(current)
        } else {
            super::board_items::Write::Conflict(Box::new(current))
        })
    }
}

/// Template body for one item type, if the project has one.
pub(super) fn item_template(
    conn: &Connection,
    project_id: &str,
    item_type: &str,
) -> rusqlite::Result<Option<Value>> {
    conn.query_row(
        "SELECT body FROM template WHERE project_id = ?1 AND kind = 'item_type' AND name = ?2
         ORDER BY version DESC LIMIT 1",
        params![project_id, item_type],
        |r| from_json::<Value>(r.get(0)?),
    )
    .optional()
}

/// When an item entered its column, for the stale flag.
pub(super) fn parse_at(text: String) -> chrono::DateTime<chrono::Utc> {
    parse_ts(&text)
}

pub(super) fn columns_in(
    conn: &Connection,
    project_id: &str,
) -> rusqlite::Result<Vec<BoardColumn>> {
    let rows = conn
        .prepare(
            "SELECT project_id, key, name, ord, category, wip_limit, wip_scope, visible,
                    board_id, kind, movers, rules, allowed_from
             FROM board_column WHERE project_id = ?1 ORDER BY ord",
        )?
        .query_map(params![project_id], |r| {
            let category: crate::kanban::model::ColumnCategory = from_text(r.get(4)?)?;
            Ok(BoardColumn {
                project_id: r.get(0)?,
                board_id: r.get::<_, Option<String>>(8)?.unwrap_or_default(),
                key: r.get(1)?,
                name: r.get(2)?,
                ord: r.get(3)?,
                category,
                kind: r
                    .get::<_, Option<String>>(9)?
                    .map(from_text)
                    .transpose()?
                    .unwrap_or_else(|| category.kind()),
                wip_limit: r.get(5)?,
                wip_scope: from_text(r.get(6)?)?,
                visible: r.get(7)?,
                policy: crate::kanban::flow::ColumnPolicy {
                    movers: r
                        .get::<_, Option<String>>(10)?
                        .map(from_json)
                        .transpose()?
                        .unwrap_or_default(),
                    rules: from_json(r.get(11)?)?,
                    allowed_from: from_json(r.get(12)?)?,
                },
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

/// A project's role rows. A row whose role this build doesn't know (written
/// by a newer daemon, read after a rollback) is skipped with a warning, so
/// it can't fail every guard and role check that reads the project's roles.
pub(super) fn roles_in(conn: &Connection, project_id: &str) -> rusqlite::Result<Vec<ProjectRole>> {
    let rows = conn
        .prepare(
            "SELECT project_id, role, bot_id, machine FROM project_role
             WHERE project_id = ?1 ORDER BY role, bot_id",
        )?
        .query_map(params![project_id], |r| {
            let text: String = r.get(1)?;
            let bot_id: String = r.get(2)?;
            let Some(role) = Role::from_text(&text) else {
                tracing::warn!(
                    project_id,
                    bot_id,
                    role = text,
                    "skipping a project role this build doesn't know"
                );
                return Ok(None);
            };
            Ok(Some(ProjectRole {
                project_id: r.get(0)?,
                role,
                bot_id,
                machine: r.get(3)?,
            }))
        })?
        .filter_map(Result::transpose)
        .collect::<Result<_, _>>()?;
    Ok(rows)
}
