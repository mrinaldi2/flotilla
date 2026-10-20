//! What a question tells people: the card's comment that announces it, who
//! hears its answer, and the asker's note when the owner dismisses it.

use std::sync::Arc;

use bus::MessageKind;

use crate::messaging::Dm;

use crate::actor::Actor;
use crate::app::AppState;

/// Comment on the card that the questions were asked, so its thread links
/// them; each question remembers the comment.
pub(super) fn announce(
    app: &AppState,
    item_id: &str,
    ids: &[String],
    actor: &Actor<'_>,
) -> anyhow::Result<()> {
    let mut lines = vec![match ids.len() {
        1 => "I asked the owner a question; answer it on the card:".to_string(),
        n => format!("I asked the owner {n} questions; answer each on the card:"),
    }];
    for (i, id) in ids.iter().enumerate() {
        if let Some(d) = app.db.get_decision(id)? {
            lines.push(format!("{}. {} (`question:{id}`)", i + 1, d.title));
        }
    }
    let body = lines.join("\n");
    let comment = app
        .db
        .board_tx(|t| crate::kanban::api::comment(t, item_id, &body, None, actor))?;
    for id in ids {
        app.db.set_asked_comment(id, &comment.id)?;
    }
    Ok(())
}

/// Who hears an answer: whoever asked, whoever waits on it, and the
/// project's lead when the question is plan work.
pub(super) fn who_hears(app: &AppState, id: &str) -> anyhow::Result<Vec<String>> {
    let Some(decision) = app.db.get_decision(id)? else {
        return Ok(Vec::new());
    };
    let mut ids = crate::decisions::publish::default_notify(&decision);
    let planned = match app.db.question_row(id)? {
        Some(row) => {
            row.plan_node_id.is_some()
                || row
                    .item_id
                    .as_deref()
                    .and_then(|i| app.db.get_item(i).ok().flatten())
                    .and_then(|i| i.plan_node_id)
                    .is_some()
        }
        None => false,
    };
    if planned {
        let lead = app
            .db
            .board_read(|t| t.plan_project_lead(&decision.project_id))?;
        ids.extend(lead.filter(|l| !ids.contains(l)));
    }
    Ok(ids)
}

/// Tell the bot that asked that the owner dismissed its question.
pub(super) fn tell_dismissed(app: &Arc<AppState>, id: &str, reason: &str) -> anyhow::Result<()> {
    let Some(decision) = app.db.get_decision(id)? else {
        return Ok(());
    };
    let body = format!(
        "The owner dismissed your question “{}” ({id}) without answering it: {reason}",
        decision.title
    );
    let sender = crate::messaging::user_sender();
    crate::messaging::send_dm(
        &app.db,
        &app.events,
        Dm::new(
            &decision.raised_by_bot_id,
            &sender,
            MessageKind::Note,
            &body,
        )
        .about(id),
    )?;
    Ok(())
}
