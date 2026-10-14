//! What the keeper did, told to the owner's apps as the daemon's.

use super::apply::Done;
use super::gather::Project;
use crate::app::AppState;
use crate::db::KeeperCard;

/// Tell the owner's apps what the keeper did about `card`.
pub(super) fn told(_app: &AppState, project: &Project, card: &KeeperCard, done: &[Done]) {
    for d in done {
        tracing::info!(project_id = %project.id, item_id = %card.id, done = ?d, "board keeper");
    }
}
