//! What a bot invited to discuss a draft is told (0.19.0, after K5).
//!
//! - Invited, and invited again whenever the owner reopens the discussion,
//!   it gets one kickoff: a `chat` from the owner, which the prompt says to
//!   answer, carrying the draft as it stands, its attachments, the board's
//!   template for its type and what it may do. A `note` reaches and wakes
//!   the session the same way, but the prompt meets a note with silence and
//!   never takes work from one, so it is the wrong kind here. The same
//!   draft, unchanged, is sent once per window (`kickoffs`): a double click
//!   is one kickoff.
//! - The owner's chat about the draft then leads with a short
//!   `[draft RB-20 “title”]` tag, never the whole context again.
//! - Uninvited, it is told the discussion has ended, as a note: there is
//!   nothing to answer.
//!
//! The kickoff carries no owner proof: it is the daemon's account of the
//! owner's act, never typed into a composer, so it always arrives whole
//! through the session's inbox.

use bus::{Bot, MessageKind};
use serde_json::Value;

use super::attach::artifacts_of;
use super::attachments::{listing, listing_kept, one_line, render_refs};
use super::kickoffs::Stamp;
use super::model::{Item, TemplateKind};
use crate::app::AppState;
use crate::messaging::{self, Dm};

/// The longest description the kickoff quotes; `item_get` has all of it.
const DESCRIPTION_MAX: usize = 6000;
/// The longest title a tag shows.
const TITLE_MAX: usize = 80;

/// The lead of the owner's messages about a draft: its id and title, on
/// one line, with no bracket that could close the tag early.
pub fn tag(id: &str, title: &str) -> String {
    let title: String = one_line(title)
        .chars()
        .map(|ch| match ch {
            '[' => '(',
            ']' => ')',
            ch => ch,
        })
        .take(TITLE_MAX)
        .collect();
    format!("[draft {id} “{title}”]")
}

/// Send `text` to a bot about a draft. Delivery failing is logged, not
/// refused: the change it reports has been made. `false` when it failed.
pub(super) fn tell(app: &AppState, bot_id: &str, kind: MessageKind, text: &str) -> bool {
    let sender = messaging::user_sender();
    let dm = Dm::new(bot_id, &sender, kind, text);
    match messaging::send_dm(&app.db, &app.events, dm) {
        Ok(_) => true,
        Err(e) => {
            tracing::warn!(bot_id, error = %e, "draft message not delivered");
            false
        }
    }
}

/// Start, or start again, the discussion of `item` with `bot`, as `stamp`
/// describes it: skipped when the bot was just sent this same draft.
pub(super) fn kick_off(app: &AppState, project_id: &str, bot: &Bot, item: &Item, stamp: Stamp) {
    if !app.kickoffs.claim(&item.id, &bot.id, stamp) {
        tracing::debug!(item = %item.id, bot = %bot.id, "draft kickoff just sent; skipped");
        return;
    }
    let sent = match kickoff(app, project_id, bot, item) {
        Ok(text) => tell(app, &bot.id, MessageKind::Chat, &text),
        Err(e) => {
            tracing::warn!(item = %item.id, error = %e, "draft kickoff not built");
            false
        }
    };
    // One that never went out covers nothing: the next invite tries again.
    if !sent {
        app.kickoffs.forget(&item.id, &bot.id);
    }
}

/// The discussion of `item` with `bot` has ended.
pub(super) fn end(app: &AppState, bot_id: &str, item: &Item) {
    let text = format!(
        "{} The owner ended the discussion of draft {id} with you. You can no longer see or \
         change it, and there is nothing to answer.",
        tag(&item.id, &item.title),
        id = item.id
    );
    tell(app, bot_id, MessageKind::Note, &text);
}

/// The kickoff's text: what the discussion is, the draft, what the bot may
/// do. Everything the draft holds is quoted, so a line in it can't pass for
/// the message's own.
pub fn kickoff(app: &AppState, project_id: &str, bot: &Bot, item: &Item) -> anyhow::Result<String> {
    let id = &item.id;
    let atts = app.db.board_read(|t| t.attachments(id))?;
    let artifacts = artifacts_of(app, project_id)?;
    let mut out = format!(
        "{} The owner wants to discuss draft {id} with you. It isn't on the board yet: only \
         the owner and the bots invited to it see it. Here it is as it stands; item_get reads \
         it again at any time.\n\n",
        tag(id, &item.title)
    );
    let mut facts = vec![format!("Type: {}", item.item_type.as_str())];
    if let Some(size) = item.size {
        facts.push(format!("Size: {}", size.as_str()));
    }
    let labels: Vec<String> = item.labels.iter().map(|l| one_line(l)).collect();
    facts.push(format!(
        "Labels: {}",
        if labels.is_empty() {
            "none".to_string()
        } else {
            labels.join(", ")
        }
    ));
    out.push_str(&facts.join(" · "));
    out.push_str(&format!("\nTitle: {}\n", one_line(&item.title)));
    let description = if bot.is_linked() {
        item.description.clone()
    } else {
        render_refs(&item.description, &atts, &artifacts)
    };
    out.push_str(&quoted("Description", &description));
    out.push_str(&criteria(item));
    out.push_str(&if bot.is_linked() {
        listing_kept(id, &atts)
    } else {
        listing(id, &atts, &artifacts)
    });
    out.push_str(&template(app, project_id, item)?);
    out.push_str(
        "\n\nWhile you discuss it you may change it with item_update (title, description, \
         acceptance criteria, type, size, labels), add or remove files with item_attach and \
         item_detach, and link what you write with item_link. The owner sees every change. You \
         can't move it or put tasks on it: only the owner finalizes or discards it. Answer the \
         owner with message_owner.",
    );
    Ok(out)
}

/// `text` under `head`, each line quoted; "none yet" when empty.
fn quoted(head: &str, text: &str) -> String {
    let text = text.trim();
    if text.is_empty() {
        return format!("{head}: none yet\n");
    }
    let mut cut: String = text.chars().take(DESCRIPTION_MAX).collect();
    if cut.len() < text.len() {
        cut.push_str(" … (cut; item_get has all of it)");
    }
    let lines: Vec<String> = cut.lines().map(|l| format!("> {l}")).collect();
    format!("{head}:\n{}\n", lines.join("\n"))
}

fn criteria(item: &Item) -> String {
    if item.acceptance_criteria.is_empty() {
        return "Acceptance criteria: none yet".to_string();
    }
    let lines: Vec<String> = item
        .acceptance_criteria
        .iter()
        .map(|ac| {
            let mark = if ac.checked { "x" } else { " " };
            format!("- [{mark}] {}", one_line(&ac.text))
        })
        .collect();
    format!("Acceptance criteria:\n{}", lines.join("\n"))
}

/// The board's template for the item's type: its sections and what Ready
/// needs. Empty when the type has none.
fn template(app: &AppState, project_id: &str, item: &Item) -> anyhow::Result<String> {
    let kind = item.item_type.as_str();
    let Some(body) = app
        .db
        .templates(project_id, TemplateKind::ItemType)?
        .into_iter()
        .find(|t| t.name == kind)
        .map(|t| t.body)
    else {
        return Ok(String::new());
    };
    let words = |key: &str| -> Vec<String> {
        body[key]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(one_line)
            .collect()
    };
    let (sections, ready) = (words("sections"), words("ready"));
    let mut lines = Vec::new();
    if !sections.is_empty() {
        lines.push(format!("- Sections: {}", sections.join(" · ")));
    }
    if !ready.is_empty() {
        lines.push(format!("- Ready needs: {}", ready.join(", ")));
    }
    if lines.is_empty() {
        lines.push(format!("- {body}"));
    }
    Ok(format!(
        "\n\nThe board's template for a {kind}:\n{}",
        lines.join("\n")
    ))
}
