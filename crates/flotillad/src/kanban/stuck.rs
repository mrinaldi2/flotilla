//! Why a card can't move on, for its "can't move: …" line (0.19.0, card
//! flow). Two sources, neither stored:
//!
//! - the last refused move of the card, kept in memory per item with who
//!   tried; it is checked again on every read and dropped once that move
//!   would pass, the card moves, or the daemon restarts;
//! - otherwise, the next column's rules: the move to the next visible
//!   column, judged as the owner would make it, so the line names what the
//!   card lacks (a rule) rather than whose move it is.
//!
//! Codes the mover clears by typing (a reason, a WIP override) and who-may
//! refusals (a role, the daemon's own moves) never show: they say nothing
//! about the card. A column's allowed sources do (`move.from`).

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use super::feed::{card_after_commit, Change, ChangeKind};
use super::guards::Who;
use super::model::{BoardColumn, ColumnKind, ItemCard, ItemState, MoveBlock, Unmet};
use super::moves::unmet_for;
use crate::app::AppState;
use crate::db::BoardTx;

/// How long a refused move is remembered at most.
const KEEP: Duration = Duration::from_secs(24 * 60 * 60);

struct Refusal {
    to: String,
    who: Who,
    at: Instant,
}

type Key = (String, String);

fn refusals() -> &'static Mutex<HashMap<Key, Refusal>> {
    static REFUSALS: OnceLock<Mutex<HashMap<Key, Refusal>>> = OnceLock::new();
    REFUSALS.get_or_init(Mutex::default)
}

fn key(project_id: &str, item_id: &str) -> Key {
    (project_id.to_string(), item_id.to_string())
}

/// A move of the item to `to` by `who` was refused.
pub fn remember(project_id: &str, item_id: &str, to: &str, who: &Who) {
    if *who == Who::Daemon {
        return;
    }
    let mut map = refusals().lock().unwrap_or_else(|e| e.into_inner());
    let now = Instant::now();
    map.retain(|_, r| now.duration_since(r.at) < KEEP);
    map.insert(
        key(project_id, item_id),
        Refusal {
            to: to.to_string(),
            who: who.clone(),
            at: now,
        },
    );
}

/// The item moved: its last refusal no longer applies.
pub fn forget(project_id: &str, item_id: &str) {
    let mut map = refusals().lock().unwrap_or_else(|e| e.into_inner());
    map.remove(&key(project_id, item_id));
}

fn remembered(project_id: &str, item_id: &str) -> Option<(String, Who)> {
    let map = refusals().lock().unwrap_or_else(|e| e.into_inner());
    map.get(&key(project_id, item_id))
        .filter(|r| r.at.elapsed() < KEEP)
        .map(|r| (r.to.clone(), r.who.clone()))
}

/// Whether a refused move of the item is remembered (the board keeper reads
/// the ones that would still be refused through `mark`).
pub fn has_refusal(project_id: &str, item_id: &str) -> bool {
    remembered(project_id, item_id).is_some()
}

/// Whether an unmet rule says something about the card itself.
fn about_the_card(u: &Unmet) -> bool {
    let whose = ["reason.", "role.", "wip."]
        .iter()
        .any(|prefix| u.code.starts_with(prefix));
    !whose && !matches!(u.code.as_str(), "move.daemon_only" | "move.same_column")
}

fn block(to: &BoardColumn, unmet: &[Unmet], refused: bool) -> Option<MoveBlock> {
    let shown: Vec<&Unmet> = unmet.iter().filter(|u| about_the_card(u)).collect();
    let first = shown.first()?;
    Some(MoveBlock {
        to_column: to.key.clone(),
        reason: first.text.clone(),
        more: u32::try_from(shown.len() - 1).unwrap_or(u32::MAX),
        refused,
    })
}

/// Fill in each live card's `move_block`, inside the caller's read.
pub fn mark(t: &BoardTx<'_>, project_id: &str, cards: &mut [ItemCard]) -> anyhow::Result<()> {
    let all = t.columns(project_id)?;
    for card in cards.iter_mut().filter(|c| !c.draft) {
        let columns: Vec<BoardColumn> = all
            .iter()
            .filter(|c| card.board_id.is_empty() || c.board_id == card.board_id)
            .cloned()
            .collect();
        let Some(here) = columns.iter().find(|c| c.key == card.column_key) else {
            continue;
        };
        if matches!(here.kind, ColumnKind::Done | ColumnKind::Cancelled) {
            continue;
        }
        let Some(item) = t.item(&card.id)?.filter(|i| i.state == ItemState::Live) else {
            continue;
        };
        if let Some((to, who)) = remembered(project_id, &item.id) {
            let target = columns.iter().find(|c| c.key == to);
            let unmet = match target {
                Some(target) => unmet_for(t, project_id, &item, &columns, target, &who)?,
                None => Vec::new(),
            };
            match target {
                Some(target) if !unmet.is_empty() => {
                    card.move_block = block(target, &unmet, true);
                    if card.move_block.is_some() {
                        continue;
                    }
                }
                _ => forget(project_id, &item.id),
            }
        }
        let next = columns
            .iter()
            .filter(|c| c.visible && c.ord > here.ord && c.kind != ColumnKind::Cancelled)
            .min_by_key(|c| c.ord);
        if let Some(next) = next {
            let unmet = unmet_for(t, project_id, &item, &columns, next, &Who::Owner)?;
            card.move_block = block(next, &unmet, false);
        }
    }
    Ok(())
}

/// Push the item's card as it is now, so open boards show a refusal that
/// changed nothing else.
pub fn publish_card(app: &Arc<AppState>, project_id: &str, item_id: &str) {
    let mut feed = app.board.writer();
    let card = card_after_commit(&app.db, item_id);
    if card.is_none() {
        return;
    }
    feed.publish(Change {
        project_id,
        kind: ChangeKind::ItemUpserted,
        item_id,
        card,
        from_column: None,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unmet(code: &str, text: &str) -> Unmet {
        super::super::guards::unmet(code, text, None)
    }

    #[test]
    fn only_rules_about_the_card_show() {
        let col = BoardColumn {
            project_id: "p".into(),
            board_id: "b".into(),
            key: "review".into(),
            name: "Review".into(),
            ord: 2,
            category: super::super::model::ColumnCategory::Review,
            kind: ColumnKind::Active,
            wip_limit: None,
            wip_scope: super::super::model::WipScope::Column,
            visible: true,
            policy: Default::default(),
            on_entry: None,
        };
        let say_why = [
            unmet("reason.required", "Say why."),
            unmet("wip.full", "Full."),
        ];
        assert_eq!(block(&col, &say_why, true), None);
        let lacking = [
            unmet("role.not_allowed", "Only the lead."),
            unmet("dor.size", "Size it."),
            unmet("rule.link", "No pr is linked."),
        ];
        let shown = block(&col, &lacking, false).expect("a rule about the card");
        assert_eq!(shown.reason, "Size it.");
        assert_eq!(shown.more, 1);
        assert_eq!(shown.to_column, "review");
    }

    #[test]
    fn a_refusal_is_remembered_until_the_card_moves() {
        remember("p-stuck", "P-1", "doing", &Who::Owner);
        assert_eq!(
            remembered("p-stuck", "P-1").map(|(to, _)| to).as_deref(),
            Some("doing")
        );
        assert!(remembered("other", "P-1").is_none());
        forget("p-stuck", "P-1");
        assert!(remembered("p-stuck", "P-1").is_none());
        remember("p-stuck", "P-2", "doing", &Who::Daemon);
        assert!(remembered("p-stuck", "P-2").is_none());
    }
}
