//! Boards and their structure for bots (0.19.0 K4): `board_list` for every
//! member, and the lead's `board_create`, `board_update`, `column_add`,
//! `column_update`, `column_remove` and `automation_set`, which apply
//! directly or become a proposal the owner approves, as `kanban::admin`
//! decides from the board's structure policy.

use std::sync::Arc;

use bus::contract::board as c;
use serde_json::{json, Value};

use crate::app::AppState;
use crate::kanban::admin::{self, By, Outcome, Structure};
use crate::kanban::model::Role;

use super::board::{out, Me};
use super::board_schema::decode;

pub(super) const TOOLS: &[&str] = &[
    "board_list",
    "board_create",
    "board_update",
    "column_add",
    "column_update",
    "column_remove",
    "automation_set",
];

pub(super) fn handles(name: &str) -> bool {
    TOOLS.contains(&name)
}

pub(super) fn call(
    app: &Arc<AppState>,
    me: &Me,
    roles: &[Role],
    name: &str,
    args: &Value,
) -> anyhow::Result<Value> {
    let project = me.bot.project_id.as_str();
    let req = match name {
        "board_list" => return board_list(app, project),
        "board_create" => {
            Structure::Create(decode::<c::BoardCreate>("BoardCreate", args, project)?)
        }
        "board_update" => Structure::Board(decode("BoardUpdate", args, project)?),
        "column_add" => Structure::ColumnAdd(decode("ColumnAdd", args, project)?),
        "column_update" => Structure::ColumnUpdate(decode("ColumnUpdate", args, project)?),
        "column_remove" => Structure::ColumnRemove(decode("ColumnRemove", args, project)?),
        "automation_set" => Structure::Automation(decode("AutomationSet", args, project)?),
        _ => anyhow::bail!("unknown board tool {name}"),
    };
    let by = By::Bot {
        bot: &me.bot,
        roles,
    };
    match admin::structure(app, by, req)? {
        Outcome::Applied { board_id } => {
            let board = app.db.board(&board_id)?.map(c::Board::from);
            let columns: Vec<c::BoardColumn> = app
                .db
                .board_columns(project)?
                .into_iter()
                .filter(|col| col.board_id == board_id)
                .map(Into::into)
                .collect();
            Ok(json!({ "applied": true, "board": out(board)?, "columns": out(columns)? }))
        }
        Outcome::Proposed {
            board_id,
            proposal_id,
        } => Ok(json!({
            "applied": false,
            "proposal_id": proposal_id,
            "board_id": board_id,
            "note": "Proposed to the owner, who approves it in Needs you; nothing changes until then.",
        })),
    }
}

/// The project's boards, the default first, with each one's columns.
fn board_list(app: &Arc<AppState>, project: &str) -> anyhow::Result<Value> {
    let columns = app.db.board_columns(project)?;
    let boards: Vec<Value> = app
        .db
        .boards(project)?
        .into_iter()
        .filter(|b| b.archived_at.is_none())
        .map(|b| {
            let cols: Vec<c::BoardColumn> = columns
                .iter()
                .filter(|col| col.board_id == b.id)
                .cloned()
                .map(Into::into)
                .collect();
            Ok(json!({ "board": out(c::Board::from(b))?, "columns": out(cols)? }))
        })
        .collect::<anyhow::Result<_>>()?;
    Ok(json!({ "boards": boards }))
}
