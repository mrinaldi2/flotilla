//! A column's role nobody holds (0.24.0, `board_flow_v1`): one row per
//! board and role, naming the card that has waited longest, built on the
//! board's home from the cards that entered such a column (`role_wait`).
//! It clears once a bot holds the role, or once no card waits for it.

use std::collections::HashSet;

use bus::contract::home::{attention_row::Target, AttentionKind};

use super::rows::{Builder, Part};
use super::weight;
use crate::app::AppState;
use crate::kanban::model::ItemState;
use crate::kanban::notice_text::inline;
use crate::kanban::on_entry::role_name;

pub(super) fn unheld(app: &AppState, b: &mut Builder<'_>) -> anyhow::Result<()> {
    let project_id = b.project_id;
    let waits = app.db.role_waits(project_id)?;
    if waits.is_empty() {
        return Ok(());
    }
    let held: HashSet<_> = app
        .db
        .project_roles(project_id)?
        .into_iter()
        .map(|r| r.role)
        .collect();
    let columns = app.db.board_read(|t| t.columns(project_id))?;
    let mut seen = HashSet::new();
    for wait in waits {
        if held.contains(&wait.role) || seen.contains(&(wait.board_id.clone(), wait.role)) {
            continue;
        }
        // A card that moved on, or left the board, waits no more.
        let Some(item) = app.db.get_item(&wait.item_id)? else {
            continue;
        };
        if item.state != ItemState::Live || item.column_key != wait.column_key {
            continue;
        }
        let Some(column) = columns.iter().find(|c| c.key == wait.column_key) else {
            continue;
        };
        seen.insert((wait.board_id.clone(), wait.role));
        let card = format!("{} “{}”", item.id, inline(&item.title, 60));
        let part = Part {
            kind: AttentionKind::RoleUnheld,
            target_id: format!("{}:{}", wait.board_id, wait.role.as_str()),
            title: format!(
                "Nobody holds {}: {card} waits in {}",
                role_name(wait.role),
                column.name
            ),
            created_at: wait.since,
            target: Some(Target::Role(wait.role.as_str().to_string())),
        };
        b.push(part, weight(AttentionKind::RoleUnheld), None);
    }
    Ok(())
}
