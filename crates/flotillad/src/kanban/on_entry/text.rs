//! What the holder a column gave a card to is told (0.24.0): which card,
//! that the column's role is why it is theirs, what that role does here,
//! and where it moves the card when done. Always told, as an assignment is,
//! whoever moved the card, the daemon too.

use bus::MessageKind;

use super::Entry;
use crate::actor::Actor;
use crate::app::AppState;
use crate::kanban::model::{BoardColumn, ColumnKind, Item, Role};
use crate::kanban::notice_text::{inline, NAME_MAX};
use crate::messaging::{self, Dm};

/// A role as the bots and the owner read it.
pub fn role_name(role: Role) -> &'static str {
    match role {
        Role::Product => "Product",
        Role::Architect => "Architect",
        Role::Dev => "Developer",
        Role::Tester => "Tester",
        Role::Lead => "Lead",
        Role::Devops => "DevOps",
        Role::Coach => "Coach",
        other => other.as_str(),
    }
}

/// The parts of the board the text names.
pub struct Route<'a> {
    pub column: &'a BoardColumn,
    /// The next visible column, where the role moves the card when done.
    pub next: Option<&'a BoardColumn>,
    /// Where a Tester sends a failed card: the column before that assigns
    /// the Developer, else the active column before.
    pub back: Option<&'a BoardColumn>,
}

impl<'a> Route<'a> {
    pub fn of(column: &'a BoardColumn, columns: &'a [BoardColumn]) -> Self {
        let mut visible: Vec<&BoardColumn> = columns
            .iter()
            .filter(|c| c.visible && c.kind != ColumnKind::Cancelled)
            .collect();
        visible.sort_by_key(|c| c.ord);
        let next = visible.iter().find(|c| c.ord > column.ord).copied();
        let before = visible.iter().rev().filter(|c| c.ord < column.ord);
        let back = before
            .clone()
            .find(|c| c.on_entry == Some(Role::Dev))
            .or_else(|| before.clone().find(|c| c.kind == ColumnKind::Active))
            .copied();
        Route { column, next, back }
    }
}

/// The notice: `[card KEY-12]` first. `by` is the mover's name, `None` for
/// the daemon; `moved` is false for a card that started in the column.
pub fn text(
    item: &Item,
    route: &Route<'_>,
    role: Role,
    by: Option<&str>,
    moved: bool,
    sent_back: Option<&str>,
) -> String {
    let id = &item.id;
    let named = format!("{id} “{}”", inline(&item.title, NAME_MAX));
    let column = inline(&route.column.name, NAME_MAX);
    let came = match (by, moved) {
        (Some(by), true) => format!("{by} moved {named} to {column}"),
        (None, true) => format!("{named} moved to {column}"),
        (Some(by), false) => format!("{by} put {named} in {column}"),
        (None, false) => format!("{named} is in {column}"),
    };
    let name = role_name(role);
    let mut out = format!(
        "[card {id}] {came}, where the {name} takes it: it is assigned to you now, as the \
         project's {name}."
    );
    if let Some(reason) = sent_back {
        out.push_str(&format!(" It was sent back: “{}”.", inline(reason, 300)));
    }
    out.push(' ');
    out.push_str(&work(role, route));
    out.push_str(&format!(
        " Read it with item_get first. If anything is unclear, ask the owner with \
         message_owner (asks: true, item: {id})."
    ));
    out
}

/// What the role does with the card here, and where it moves it next.
fn work(role: Role, route: &Route<'_>) -> String {
    let next = route.next.map(|c| inline(&c.name, NAME_MAX));
    let then = match &next {
        Some(next) => format!(" Then move it to {next} with item_move."),
        None => String::new(),
    };
    match role {
        Role::Product => format!(
            "Shape it: say why it matters and when it is done, and write its acceptance \
             criteria (item_update).{then}"
        ),
        Role::Architect => format!(
            "Read the code it touches and write what you found under “Code research” (or link \
             a research document), with design notes where it needs them.{then}"
        ),
        Role::Dev => format!(
            "Build it: work in its worktree, link the branch or PR, and comment the \
             result.{then}"
        ),
        Role::Tester => {
            let pass = match &next {
                Some(next) => format!(" If it passes, move it to {next}."),
                None => String::new(),
            };
            let fail = match route.back {
                Some(back) => format!(
                    " If it fails, move it back to {} with item_move and a reason that says \
                     what failed: the Developer gets it with your reason.",
                    inline(&back.name, NAME_MAX)
                ),
                None => " If it fails, say what failed in a comment.".to_string(),
            };
            format!(
                "Check it against its acceptance criteria, ticking or failing each with \
                 item_check_ac.{pass}{fail}"
            )
        }
        other => format!(
            "Do its {} step and comment the result on the card.{then}",
            role_name(other)
        ),
    }
}

/// Tell `item`'s new assignee. A failed delivery is logged; the assignment
/// stands.
pub(super) fn tell(
    app: &AppState,
    item: &Item,
    column: &BoardColumn,
    columns: &[BoardColumn],
    role: Role,
    entry: Entry<'_>,
    sent_back: Option<&str>,
) {
    let Some(bot_id) = item.assignee.as_deref() else {
        return;
    };
    if !matches!(app.db.get_live_bot(bot_id), Ok(Some(_))) {
        return;
    }
    let (sender, by) = match entry.by {
        Actor::Daemon => (messaging::daemon_sender(), None),
        Actor::Bot { id, .. } => match app.db.get_bot(id) {
            Ok(Some(bot)) => (crate::mcp::bot_sender(&bot), Some(bot.name)),
            _ => (messaging::daemon_sender(), None),
        },
        Actor::User | Actor::Device(_) => (messaging::user_sender(), Some("The owner".to_string())),
    };
    let route = Route::of(column, columns);
    let body = text(
        item,
        &route,
        role,
        by.as_deref(),
        entry.from.is_some(),
        sent_back,
    );
    let dm = Dm::new(bot_id, &sender, MessageKind::Note, &body);
    if let Err(e) = messaging::send_dm(&app.db, &app.events, dm) {
        tracing::warn!(bot_id, item_id = %item.id, error = %e, "on-entry notice not delivered");
    }
}
