//! Settling a project's plan when one of its cards changes (0.21.0): a
//! card of a node reaching Done may release its feature, and the feature
//! its epic and initiative, without waiting for the next read.

use std::sync::Arc;

use tokio::sync::broadcast::error::RecvError;

use super::Planner;
use crate::actor::Actor;
use crate::app::AppState;
use crate::kanban::feed::ChangeKind;

pub fn spawn(app: Arc<AppState>) {
    let mut rx = app.board.subscribe();
    tokio::spawn(async move {
        loop {
            let change = match rx.recv().await {
                Ok(change) => change,
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => break,
            };
            let ours = matches!(
                change.kind,
                ChangeKind::ItemMoved | ChangeKind::ItemUpserted
            ) && change
                .card
                .as_ref()
                .is_some_and(|card| card.plan_node_id.is_some());
            if !ours {
                continue;
            }
            let app = app.clone();
            let project_id = change.project_id.clone();
            tokio::task::spawn_blocking(move || {
                let planner = Planner {
                    app: &app,
                    actor: Actor::Daemon,
                };
                if let Err(e) = planner.settle(&project_id) {
                    tracing::warn!(project_id, error = %e, "plan not settled after a card change");
                }
            });
        }
    });
}
