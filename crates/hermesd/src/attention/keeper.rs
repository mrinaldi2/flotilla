//! The board keeper's rows (0.19.0): a card waiting for the owner's review,
//! one nothing happened on, and one whose move is refused. Built on the
//! board's home, from `kanban::keeper::waiting`.

use bus::contract::home::{attention_row::Target, AttentionKind};
use chrono::Utc;

use super::rows::{Builder, Part};
use super::weight;
use crate::app::AppState;

pub(super) fn cards_waiting(app: &AppState, b: &mut Builder<'_>) -> anyhow::Result<()> {
    for w in crate::kanban::keeper::waiting(app, b.project_id, Utc::now())? {
        let part = Part {
            kind: AttentionKind::CardWaiting,
            target_id: format!("{}:{}", w.item_id, w.why),
            title: w.title,
            created_at: w.since,
            target: Some(Target::ItemId(w.item_id)),
        };
        b.push(part, weight(AttentionKind::CardWaiting), None);
    }
    Ok(())
}
