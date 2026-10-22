//! A `nit` finding turned into its own card (H-261 §1.3; a `should` one is
//! fixed in its PR since 0.24.0): an
//! Inbox card related to the PR's card, recorded on the finding so it isn't
//! filed twice.

use std::sync::Arc;

use crate::actor::Actor;
use crate::app::AppState;
use crate::db::NewItem;
use crate::decisions::{conflict, forbidden, invalid, not_found};
use crate::kanban::model::{ItemType, LinkKind, Priority, Role};

/// `pr_follow_up`: the reviewer, the PR's author or the lead files finding
/// `index` of review `review_id` as an Inbox card; returns its id.
pub fn file(
    app: &Arc<AppState>,
    bot: &bus::Bot,
    roles: &[Role],
    number: u32,
    review_id: &str,
    index: usize,
) -> anyhow::Result<String> {
    let project = bot.project_id.as_str();
    let pr = app
        .db
        .board_read(|t| t.pr(project, number))?
        .ok_or_else(|| not_found(format!("no PR #{number} in this project")))?;
    let review = app
        .db
        .board_read(|t| t.review(review_id))?
        .filter(|r| r.pr_id == pr.id)
        .ok_or_else(|| not_found(format!("no review {review_id} on PR #{number}")))?;
    if review.reviewer != bot.id && pr.author != bot.id && !roles.contains(&Role::Lead) {
        return Err(forbidden(
            "the reviewer, the PR's author or the lead files a follow-up",
        ));
    }
    let finding = review
        .findings
        .get(index)
        .ok_or_else(|| invalid(format!("the review has no finding {index}")))?;
    if finding.severity != "nit" {
        return Err(invalid(format!(
            "a `{}` finding is fixed in this PR, not filed for later",
            finding.severity
        )));
    }
    if let Some(id) = &finding.follow_up_item_id {
        return Err(conflict(format!("that finding is already {id}")));
    }
    let title: String = finding.text.trim().chars().take(80).collect();
    let where_ = match (&finding.path, finding.line) {
        (Some(path), Some(line)) => format!(" ({path}:{line})"),
        (Some(path), None) => format!(" ({path})"),
        _ => String::new(),
    };
    let description = format!(
        "## Why\nA {} finding from the {} review of PR #{} ({}){where_}.\n\n## What\n{}",
        finding.severity,
        review.role,
        pr.number,
        pr.item_id,
        finding.text.trim()
    );
    let actor = Actor::Bot {
        id: &bot.id,
        project_id: project,
    };
    let mut findings = review.findings.clone();
    let item = app.db.board_tx(|t| {
        let item = crate::kanban::api::create(
            t,
            &NewItem {
                project_id: project,
                board_id: None,
                item_type: ItemType::Task,
                title: &title,
                description: &description,
                size: None,
                priority: Priority::P2,
                labels: &["follow-up".to_string()],
                parent_id: None,
                acceptance_criteria: &[],
                plan_node_id: None,
            },
            &actor,
        )?;
        crate::kanban::api::link(
            t,
            &item.id,
            LinkKind::ItemRelates,
            &pr.item_id,
            None,
            &actor,
        )?;
        findings[index].follow_up_item_id = Some(item.id.clone());
        t.set_review_findings(&review.id, &findings)?;
        Ok(item)
    })?;
    Ok(item.id)
}
