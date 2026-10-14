//! The owner's Needs-you rows the keeper raises, read on demand: a card in
//! a review column (rule 4), a card nothing happened on (rule 1) and a card
//! whose move is refused (rule 5). Only on the board's home, and only while
//! the keeper is on for the project; each row goes when its card moves on.

use chrono::{DateTime, Utc};

use super::gather;
use super::rules::{holds, Role, Rule};
use super::text;
use crate::app::AppState;

/// One card waiting on the owner.
#[derive(Debug, Clone, PartialEq)]
pub struct Waiting {
    pub item_id: String,
    /// `review`, `quiet` or `blocked`: one row per card and reason.
    pub why: &'static str,
    pub title: String,
    pub since: DateTime<Utc>,
}

/// The project's cards waiting on the owner, as of `now`.
pub fn waiting(
    app: &AppState,
    project_id: &str,
    now: DateTime<Utc>,
) -> anyhow::Result<Vec<Waiting>> {
    let Some(project) = app.db.get_project(project_id)? else {
        return Ok(Vec::new());
    };
    if !app.cfg.keeper.enabled
        || !app.cfg.keeper.for_project(&project.name).enabled
        || !crate::attention::is_home(app, project_id)
    {
        return Ok(Vec::new());
    }
    let loaded = gather::load(app, project_id)?;
    let mut out = Vec::new();
    for card in &loaded.cards {
        if loaded.role(card) == Role::Review {
            out.push(Waiting {
                item_id: card.id.clone(),
                why: "review",
                title: text::review_row(&card.id, &card.title),
                since: card.entered_at,
            });
        }
        let column = loaded
            .column_of(card)
            .map_or(card.column_key.as_str(), |c| c.name.as_str());
        for m in loaded.marks.iter().filter(|m| m.item_id == card.id) {
            let Some(fired) = m.fired_at.filter(|_| holds(m, card)) else {
                continue;
            };
            if m.rule == Rule::SilentStale.name() {
                out.push(Waiting {
                    item_id: card.id.clone(),
                    why: "quiet",
                    title: text::quiet_row(&card.id, &card.title, column, now - m.since),
                    since: fired,
                });
            } else if m.rule == Rule::Blocked.name() {
                out.push(Waiting {
                    item_id: card.id.clone(),
                    why: "blocked",
                    title: text::blocked_row(&card.id, &m.detail),
                    since: m.since,
                });
            }
        }
    }
    Ok(out)
}
