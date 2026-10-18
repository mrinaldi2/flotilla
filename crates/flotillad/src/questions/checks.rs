//! What a bot's ask must be: a card and node of its own project, and each
//! question with text, distinct options, a recommendation among them, and
//! a node when it blocks.

use bus::contract::board as c;

use super::refused;
use crate::app::AppState;

/// Why the card or node can't be asked about, if it can't.
pub(super) fn anchor_problem(
    app: &AppState,
    project_id: &str,
    item_id: Option<&str>,
    node_id: Option<&str>,
) -> anyhow::Result<Option<c::QuestionResult>> {
    if let Some(item) = item_id {
        match app.db.board_read(|t| t.item_project(item))? {
            Some(found) if found == project_id => {}
            _ => {
                return Ok(Some(refused(
                    "question.no_card",
                    format!("no card {item} in your project"),
                )))
            }
        }
    }
    if let Some(node) = node_id {
        match app.db.board_read(|t| t.plan_node(node))? {
            Some(found) if found.project_id == project_id => {}
            _ => {
                return Ok(Some(refused(
                    "question.no_node",
                    format!("no plan node {node} in your project"),
                )))
            }
        }
    }
    Ok(None)
}

/// Whether the card belongs to a plan node, so a question on it can block.
pub(super) fn item_on_node(app: &AppState, item_id: Option<&str>) -> anyhow::Result<bool> {
    let Some(item) = item_id else {
        return Ok(false);
    };
    Ok(app
        .db
        .get_item(item)?
        .and_then(|i| i.plan_node_id)
        .is_some())
}

/// What is wrong with the `i`th question, if anything.
pub(super) fn ask_problem(
    q: &c::QuestionAsk,
    i: usize,
    on_node: bool,
) -> Option<c::QuestionResult> {
    let n = i + 1;
    if q.text.trim().is_empty() {
        return Some(refused(
            "question.empty",
            format!("question {n} has no text"),
        ));
    }
    if q.text.trim().chars().count() > bus::MAX_DECISION_TITLE_CHARS {
        return Some(refused(
            "question.too_long",
            format!(
                "question {n} is longer than {} characters: put the detail in `why`",
                bus::MAX_DECISION_TITLE_CHARS
            ),
        ));
    }
    let mut keys: Vec<&str> = q.options.iter().map(|o| o.key.trim()).collect();
    if keys.iter().any(|k| k.is_empty()) || q.options.iter().any(|o| o.label.trim().is_empty()) {
        return Some(refused(
            "question.no_option",
            format!("question {n}: every option needs a key and a label"),
        ));
    }
    keys.sort_unstable();
    keys.dedup();
    if keys.len() != q.options.len() {
        return Some(refused(
            "question.no_option",
            format!("question {n}: option keys must differ"),
        ));
    }
    if let Some(r) = q.recommendation.as_deref().filter(|r| !r.is_empty()) {
        if !keys.contains(&r) {
            return Some(refused(
                "question.no_option",
                format!("question {n} recommends {r}, which is not an option"),
            ));
        }
    }
    if q.blocking && !on_node {
        return Some(refused(
            "question.not_planned",
            format!("question {n} blocks, but neither it nor its card belongs to a plan node"),
        ));
    }
    None
}
