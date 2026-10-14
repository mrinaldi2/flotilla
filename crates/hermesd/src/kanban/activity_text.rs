//! The names and words of a board activity (0.19.0): who did it and which
//! columns, as the owner reads them, and a notification's title and body
//! ready to show, so a phone or a push relay needs no board to word it.

use bus::contract::home::{BoardActivity, BoardActivityKind as Kind};

use super::feed_by::DAEMON;
use crate::app::AppState;

/// Fill in `activity`'s names, time and words from what this daemon knows.
pub(super) fn fill(app: &AppState, activity: &mut BoardActivity) {
    let db = &app.db;
    let item = db.get_item(&activity.item_id).ok().flatten();
    if activity.item_title.is_empty() {
        if let Some(item) = &item {
            activity.item_title = item.title.clone();
        }
    }
    activity.actor_name = actor_name(app, &activity.actor);
    if !activity.assignee.is_empty() {
        activity.assignee_name = bot_name(app, &activity.assignee);
    }
    let columns = db
        .board_read(|t| t.columns(&activity.project_id))
        .unwrap_or_default();
    let board = item.map(|i| i.board_id).unwrap_or_default();
    let name = |key: &str| -> String {
        columns
            .iter()
            .find(|c| c.key == key && (board.is_empty() || c.board_id == board))
            .map_or_else(|| key.to_string(), |c| c.name.clone())
    };
    activity.from_name = name(&activity.from_column);
    activity.to_name = name(&activity.to_column);
    activity.at = Some(crate::attention::timestamp(chrono::Utc::now()));
    let (title, body) = words(activity);
    activity.title = title;
    activity.body = body;
}

fn bot_name(app: &AppState, bot_id: &str) -> String {
    app.db
        .get_bot(bot_id)
        .ok()
        .flatten()
        .map_or_else(|| "A bot".to_string(), |b| b.name)
}

fn actor_name(app: &AppState, actor: &str) -> String {
    if let Some(bot_id) = actor.strip_prefix("bot:") {
        return bot_name(app, bot_id);
    }
    if let Some(device_id) = actor.strip_prefix("device:") {
        return app
            .db
            .get_device(device_id)
            .ok()
            .flatten()
            .map_or_else(|| "a paired device".to_string(), |d| d.name);
    }
    String::new()
}

/// Who did it, at the start of a sentence.
fn who(activity: &BoardActivity) -> String {
    match activity.actor.as_str() {
        "user" => "You".to_string(),
        a if a.starts_with("device:") => format!("You, on {},", activity.actor_name),
        a if a.starts_with("bot:") => activity.actor_name.clone(),
        _ => String::new(),
    }
}

/// A notification's title and body.
pub(super) fn words(a: &BoardActivity) -> (String, String) {
    let id = &a.item_id;
    let who = who(a);
    let quoted = if a.item_title.is_empty() {
        String::new()
    } else {
        format!("\u{201c}{}\u{201d}", a.item_title)
    };
    let with = |parts: &[&str]| -> String {
        parts
            .iter()
            .filter(|p| !p.is_empty())
            .copied()
            .collect::<Vec<_>>()
            .join(" · ")
    };
    let by_daemon = a.actor == DAEMON || who.is_empty();
    match Kind::try_from(a.kind).unwrap_or(Kind::Unspecified) {
        Kind::Moved => {
            let title = if by_daemon {
                format!("{id} moved")
            } else {
                format!("{who} moved {id}")
            };
            let route = format!("{} \u{2192} {}", a.from_name, a.to_name);
            (title, with(&[&quoted, &route, &a.note]))
        }
        Kind::Assigned if a.actor == format!("bot:{}", a.assignee) => {
            (format!("{} picked up {id}", a.assignee_name), quoted)
        }
        Kind::Assigned => {
            let title = format!("{who} assigned {id} to {}", a.assignee_name);
            (title.trim_start().to_string(), quoted)
        }
        Kind::Unassigned => (
            format!("{who} unassigned {id}").trim_start().to_string(),
            quoted,
        ),
        Kind::MoveRefused => {
            let title = format!("{who} couldn't move {id} to {}", a.to_name);
            (
                title.trim_start().to_string(),
                with(&[&quoted, &a.unmet.join(" ")]),
            )
        }
        Kind::DraftUpdated => {
            let title = format!("{who} updated draft {id}");
            (title.trim_start().to_string(), with(&[&quoted, &a.note]))
        }
        // The board keeper: a note to the assignee, or a row for the owner.
        Kind::Checked if a.assignee_name.is_empty() => {
            (format!("{id} needs you"), with(&[&quoted, &a.note]))
        }
        Kind::Checked => (
            format!("Asked {} about {id}", a.assignee_name),
            with(&[&quoted, &a.note]),
        ),
        Kind::Unspecified => (id.clone(), quoted),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn activity(kind: Kind, actor: &str) -> BoardActivity {
        BoardActivity {
            item_id: "P2-001".into(),
            item_title: "Test".into(),
            kind: kind as i32,
            actor: actor.into(),
            actor_name: "New Bot".into(),
            from_name: "To do".into(),
            to_name: "Doing".into(),
            ..Default::default()
        }
    }

    #[test]
    fn each_kind_says_who_did_what_to_which_card() {
        let moved = activity(Kind::Moved, "bot:b1");
        assert_eq!(
            words(&moved),
            (
                "New Bot moved P2-001".to_string(),
                "\u{201c}Test\u{201d} · To do \u{2192} Doing".to_string()
            )
        );
        let fact = BoardActivity {
            note: "PR merged".into(),
            ..activity(Kind::Moved, DAEMON)
        };
        assert_eq!(words(&fact).0, "P2-001 moved");
        assert!(words(&fact).1.ends_with("· PR merged"));
        let picked = BoardActivity {
            assignee: "b1".into(),
            assignee_name: "New Bot".into(),
            ..activity(Kind::Assigned, "bot:b1")
        };
        assert_eq!(words(&picked).0, "New Bot picked up P2-001");
        let refused = BoardActivity {
            unmet: vec!["Only the owner can.".into()],
            ..activity(Kind::MoveRefused, "bot:b1")
        };
        assert_eq!(words(&refused).0, "New Bot couldn't move P2-001 to Doing");
        assert!(words(&refused).1.ends_with("Only the owner can."));
        let draft = BoardActivity {
            note: "description".into(),
            ..activity(Kind::DraftUpdated, "bot:b1")
        };
        assert_eq!(words(&draft).0, "New Bot updated draft P2-001");
        assert_eq!(words(&activity(Kind::Moved, "user")).0, "You moved P2-001");
        let asked = BoardActivity {
            assignee: "b1".into(),
            assignee_name: "New Bot".into(),
            note: "In Doing, nothing has happened for 2 h".into(),
            ..activity(Kind::Checked, DAEMON)
        };
        assert_eq!(words(&asked).0, "Asked New Bot about P2-001");
        assert!(words(&asked)
            .1
            .ends_with("· In Doing, nothing has happened for 2 h"));
        let row = BoardActivity {
            note: "Can't move to Done".into(),
            ..activity(Kind::Checked, DAEMON)
        };
        assert_eq!(words(&row).0, "P2-001 needs you");
    }
}
