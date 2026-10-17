//! After the 0.21 card types migration removed the epic and feature cards
//! (#43): each one's attachment folder goes, and its assignee is told once,
//! so no bot keeps working on a card that is gone.

use std::sync::Arc;

use bus::MessageKind;

use crate::app::AppState;
use crate::db::card_types::RetiredCard;
use crate::messaging::{self, Dm};

pub fn spawn(app: Arc<AppState>) {
    tokio::task::spawn_blocking(move || {
        if let Err(e) = handle(&app) {
            tracing::warn!(error = %e, "removed cards not cleaned up");
        }
    });
}

fn handle(app: &AppState) -> anyhow::Result<()> {
    let cards = app.db.retired_cards_pending()?;
    if !cards.is_empty() {
        tracing::info!(
            count = cards.len(),
            "cleaning up the epic and feature cards 0.21 removed"
        );
    }
    for card in cards {
        remove_files(app, &card);
        tell(app, &card);
        app.db.retired_card_handled(&card.id)?;
    }
    Ok(())
}

fn remove_files(app: &AppState, card: &RetiredCard) {
    let Ok(Some(project)) = app.db.get_project(&card.project_id) else {
        return;
    };
    let artifacts = crate::paths::artifacts_dir(&app.cfg, &project.dir_name);
    let dir = super::attachments::item_dir(&artifacts, &card.id);
    match std::fs::remove_dir_all(&dir) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => tracing::warn!(item_id = %card.id, error = %e, "removed card's files kept"),
    }
}

fn tell(app: &AppState, card: &RetiredCard) {
    let Some(bot_id) = card.assignee.as_deref() else {
        return;
    };
    if !matches!(app.db.get_live_bot(bot_id), Ok(Some(_))) {
        return;
    }
    let text = format!(
        "{} “{}” ({}) was removed from the board: in 0.21 features and epics live in the \
         project's plan, not on cards. Stop working on it; the owner recreates it through the plan.",
        card.id, card.title, card.item_type
    );
    let sender = messaging::daemon_sender();
    let dm = Dm::new(bot_id, &sender, MessageKind::Note, &text);
    if let Err(e) = messaging::send_dm(&app.db, &app.events, dm) {
        tracing::warn!(bot_id, item_id = %card.id, error = %e, "removed card notice not delivered");
    }
}
