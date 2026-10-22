//! Changes asked for on a PR (0.23.5): by the owner or by a bot reviewer.
//! The bot that opened the PR gets a task to fix it, and the card's
//! assignee is told when that is another bot, so the request never waits
//! on nobody.

use std::sync::Arc;

use bus::{MessageKind, DEFAULT_TASK_DEADLINE_HOURS};

use crate::actor::Actor;
use crate::app::AppState;
use crate::messaging::{self, daemon_sender, Dm};
use crate::prs::model::Pr;

/// `by` is who asked: "The owner", or the reviewer's role.
pub fn requested(app: &Arc<AppState>, pr: &Pr, by: &str, summary: &str) {
    if let Err(e) = tell(app, pr, by, summary) {
        tracing::warn!(pr = pr.number, error = %e, "couldn't hand the requested changes on");
    }
}

fn tell(app: &Arc<AppState>, pr: &Pr, by: &str, summary: &str) -> anyhow::Result<()> {
    let ask = format!(
        "{by} asked for changes on PR #{} ({}: {}): {}\n\nRead them with pr_get {{number: {}}} \
         and pr_comments. Fix them on {}, push, report the push with pr_push, and the reviews \
         start again. The PR stays open meanwhile; don't open another.",
        pr.number,
        pr.item_id,
        pr.title,
        summary.trim(),
        pr.number,
        pr.branch,
    );
    let sender = daemon_sender();
    let author = app
        .db
        .get_bot(&pr.author)?
        .filter(|b| b.deleted_at.is_none());
    if let Some(author) = &author {
        let msg = messaging::send_dm(
            &app.db,
            &app.events,
            Dm::new(&author.id, &sender, MessageKind::Task, &ask),
        )?;
        let deadline = chrono::Utc::now() + chrono::Duration::hours(DEFAULT_TASK_DEADLINE_HOURS);
        let task = app
            .db
            .create_task(&msg.id, None, &author.id, Some(deadline), 1, "")?;
        app.db.link_task_item(&task.id, &pr.item_id, &Actor::User)?;
    }
    let assignee = app.db.get_item(&pr.item_id)?.and_then(|i| i.assignee);
    if let Some(assignee) = assignee.filter(|a| author.as_ref().is_none_or(|b| &b.id != a)) {
        let who = author
            .as_ref()
            .map_or("nobody: its author is gone".to_string(), |b| {
                format!("{} has the task", b.name)
            });
        let note = format!(
            "{by} asked for changes on PR #{} ({}), your card: {}\n\n{who}.",
            pr.number,
            pr.item_id,
            summary.trim()
        );
        messaging::send_dm(
            &app.db,
            &app.events,
            Dm::new(&assignee, &sender, MessageKind::Note, &note),
        )?;
    }
    Ok(())
}
