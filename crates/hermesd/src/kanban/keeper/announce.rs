//! What the keeper did, told to the owner's apps as the daemon's: a
//! `board_activity` push of kind `CHECKED`, actor `daemon`, which the apps
//! show as "Automatic". A note names the bot it went to (`assignee`); a row
//! names none. The words come from the mark the keeper just wrote.

use bus::contract::home::{BoardActivity, BoardActivityKind};

use super::apply::Done;
use super::gather::Project;
use super::rules::Rule;
use super::text::span;
use crate::app::AppState;
use crate::db::{KeeperCard, KeeperMark};
use crate::kanban::feed_by::DAEMON;

/// Tell the owner's apps what the keeper did about `card`.
pub(super) fn told(app: &AppState, project: &Project, card: &KeeperCard, done: &[Done]) {
    for d in done {
        let (rule, to_bot) = match *d {
            Done::Told(rule) => (rule, true),
            Done::Raised(rule) => (rule, false),
        };
        let mark = app.db.keeper_mark(&card.id, rule.name()).ok().flatten();
        let column = project
            .column_of(card)
            .map_or(card.column_key.as_str(), |c| c.name.as_str());
        let activity = BoardActivity {
            project_id: project.id.clone(),
            item_id: card.id.clone(),
            kind: BoardActivityKind::Checked as i32,
            actor: DAEMON.to_string(),
            item_title: card.title.clone(),
            from_column: card.column_key.clone(),
            to_column: card.column_key.clone(),
            assignee: if to_bot {
                card.assignee.clone().unwrap_or_default()
            } else {
                String::new()
            },
            note: note(rule, column, mark.as_ref()),
            ..Default::default()
        };
        crate::kanban::activity::by_daemon(app, activity);
    }
}

/// Why, in a few words.
fn note(rule: Rule, column: &str, mark: Option<&KeeperMark>) -> String {
    let quiet = mark
        .and_then(|m| m.fired_at.map(|at| span(at - m.since)))
        .unwrap_or_default();
    match rule {
        Rule::SilentNudged | Rule::SilentStale => {
            format!("In {column}, nothing has happened for {quiet}")
        }
        Rule::NotStarted => format!("Assigned and not started, in {column}"),
        Rule::NotMoved => format!("Changed in its last turn, still in {column}"),
        Rule::Blocked => format!(
            "Can't move to {}",
            mark.map_or("its next column", |m| m.detail.as_str())
        ),
        Rule::Stale => format!("Stale in {column}"),
    }
}
