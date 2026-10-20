//! Why a delivery wakes its bot (0.23.0), and the card it names, read when
//! the wake is asked for: that sets its place in the queue for a slot.

use bus::{Bot, Message, SenderKind};

use super::team::{WakeCard, WakeReason, WakeRequest};
use crate::db::Db;
use crate::kanban::model::{ColumnCategory, ItemType, Priority};

/// The wake a message waiting for `bot` asks for.
pub fn for_message(db: &Db, bot: &Bot, msg: &Message) -> WakeRequest {
    let lead = db
        .get_project(&bot.project_id)
        .ok()
        .flatten()
        .and_then(|p| p.lead_bot_id)
        .is_some_and(|lead| lead == bot.id);
    if lead {
        return WakeRequest::new(WakeReason::Lead);
    }
    let task = db.open_task_for_message(&msg.id, &bot.id).ok().flatten();
    let item = match super::card_named(&msg.body) {
        Some(id) => Some(id.to_string()),
        None => task
            .as_ref()
            .and_then(|t| db.task_item(&t.id).ok().flatten()),
    };
    let reason = match msg.sender.kind {
        SenderKind::Routine => WakeReason::Routine,
        // The owner's own chat is the owner waking it.
        SenderKind::User if msg.kind == bus::MessageKind::Chat => WakeReason::Owner,
        _ if task.is_some() => WakeReason::Task,
        _ if item.is_some() => WakeReason::Card,
        _ => WakeReason::Message,
    };
    request(db, reason, item.as_deref())
}

/// A wake for `reason`, with the card `item_id` names as it is now. A card
/// in review is a review request.
pub fn request(db: &Db, reason: WakeReason, item_id: Option<&str>) -> WakeRequest {
    let item = item_id.and_then(|id| db.get_item(id).ok().flatten());
    let Some(item) = item else {
        return WakeRequest::new(reason);
    };
    let in_review = matches!(
        item.category,
        ColumnCategory::Review | ColumnCategory::Verify
    );
    let reason = match reason {
        WakeReason::Card | WakeReason::Message if in_review => WakeReason::Review,
        reason => reason,
    };
    WakeRequest {
        reason,
        card: Some(WakeCard {
            urgent: item.item_type == ItemType::Bug && item.priority == Priority::P0,
            priority: item.priority.as_str().to_string(),
            item_id: item.id,
            created_at: Some(item.created_at),
        }),
    }
}
