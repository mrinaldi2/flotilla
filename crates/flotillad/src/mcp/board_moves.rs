//! Moving a card over MCP: `item_move`, then the column's on-entry
//! assignment (0.24.0), and `item_move_check`.

use std::sync::Arc;

use bus::contract::board as c;
use serde_json::{json, Value};

use super::board::{conflict, out, own_item, published, refused, Me};
use crate::app::AppState;
use crate::kanban::feed::ChangeKind;
use crate::kanban::moves::{item_move as move_item, item_move_check, MoveRequest, Moved};
use crate::kanban::on_entry;

pub(super) fn item_move(app: &Arc<AppState>, me: &Me, req: c::ItemMove) -> anyhow::Result<Value> {
    own_item(app, me, &req.id)?;
    let request = MoveRequest {
        id: &req.id,
        to: &req.to,
        expected_version: req.expected_version,
        reason: req.reason.as_deref(),
        override_reason: req.override_reason.as_deref(),
    };
    let from = app.db.get_item(&req.id)?.map(|item| item.column_key);
    let left = from.clone();
    let project = me.bot.project_id.as_str();
    let mut after = None;
    let reply = published(
        app,
        project,
        ChangeKind::ItemMoved,
        from,
        || match move_item(&app.db, &request, &me.actor())? {
            Moved::Done(item) => {
                after = Some((*item).clone());
                Ok((super::board_edit::moved(*item)?, req.id.clone()))
            }
            Moved::Refused(unmet) => {
                crate::kanban::activity::refused(app, &req.id, &req.to, &me.actor(), &unmet);
                Err(refused(unmet))
            }
            Moved::Conflict(current) => Err(conflict(*current)),
        },
    );
    if let Some(item) = &after {
        let (by, reason) = (me.actor(), req.reason.as_deref());
        let entry = on_entry::Entry {
            from: left.as_deref(),
            reason,
            by: &by,
        };
        on_entry::after_move(app, &mut app.board.writer(), item, entry);
    } else if reply.is_err() {
        // A refused move shows on its card ("can't move: …").
        crate::kanban::stuck::publish_card(app, project, &req.id);
    }
    reply
}

pub(super) fn check(app: &Arc<AppState>, me: &Me, req: c::ItemMoveCheck) -> anyhow::Result<Value> {
    own_item(app, me, &req.id)?;
    let columns = item_move_check(&app.db, &req.id, &me.actor())?
        .into_iter()
        .map(|(col, unmet)| {
            let unmet: Vec<c::Unmet> = unmet.into_iter().map(c::Unmet::from).collect();
            Ok(json!({ "column": col.key, "allowed": unmet.is_empty(), "unmet": out(unmet)? }))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(json!({ "columns": columns }))
}
