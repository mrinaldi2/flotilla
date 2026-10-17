//! The keeper's words, pure: the notes it sends an assignee and the titles
//! of the owner's rows. A note says it is an automatic check, from the
//! daemon and not the owner, and that it comes once; titles, column names
//! and comments go in through `notice_text::inline`, so none can write a
//! line of its own.

use chrono::Duration;

use super::super::model::{BoardColumn, ColumnCategory, ColumnKind};
use super::super::notice_text::{inline, state_word, Plan, NAME_MAX};

/// The card a note or a row is about.
pub struct About<'a> {
    pub id: &'a str,
    pub title: &'a str,
    pub column: &'a BoardColumn,
    /// The board's plan; none on a release-gated board.
    pub plan: Option<&'a Plan>,
}

/// How a note ends: who sends it, and that it won't be sent again.
const FROM: &str = "This check is automatic: it comes from the daemon, not the owner, and only \
                    once.";

/// "45 min", "2 h", "3 days".
pub fn span(d: Duration) -> String {
    let minutes = d.num_minutes().max(1);
    match minutes {
        m if m < 60 => format!("{m} min"),
        m if m < 48 * 60 => format!("{} h", m / 60),
        m => format!("{} days", m / (24 * 60)),
    }
}

impl About<'_> {
    fn head(&self) -> String {
        format!("[card {}] Automatic check:", self.id)
    }

    fn named(&self) -> String {
        format!("{} “{}”", self.id, inline(self.title, NAME_MAX))
    }

    fn column_name(&self) -> String {
        inline(&self.column.name, NAME_MAX)
    }

    fn place(&self) -> String {
        format!("{} ({})", self.column_name(), state_word(self.column.kind))
    }

    fn finish(&self) -> Option<&str> {
        self.plan.and_then(|p| p.finish.as_deref())
    }

    /// In a Research column (0.21.0): where the card goes once its code
    /// research is recorded.
    fn after_research(&self) -> Option<&str> {
        (self.column.category == ColumnCategory::Research)
            .then(|| self.plan.and_then(|p| p.then.as_deref()))
            .flatten()
    }

    fn ask_owner(&self) -> String {
        let back = match self.plan.and_then(|p| p.back.as_deref()) {
            Some(back) => format!("move it back to {back} and "),
            None => String::new(),
        };
        format!(
            "If it can't go on, {back}ask the owner with message_owner (asks: true, item: {}).",
            self.id
        )
    }
}

/// Rule 1: quiet in its work column.
pub fn nudge(a: &About<'_>, quiet: Duration) -> String {
    let done = match (a.after_research(), a.finish()) {
        (Some(then), _) => format!(
            "If you have read the code, write what you found under “Code research” and move it \
             to {then} with item_move."
        ),
        (None, finish) => match finish {
            Some(finish) => format!(
                "If the work is done, comment the result on the card, link or attach what you \
             made, and move it to {finish} with item_move."
            ),
            None => "If the work is done, comment the result on the card and link or attach what \
                 you made."
                .to_string(),
        },
    };
    format!(
        "{} {} is assigned to you and is in {}, and nothing has happened on it for {}. {done} \
         If you are still on it, carry on. {} {FROM}",
        a.head(),
        a.named(),
        a.place(),
        span(quiet),
        a.ask_owner()
    )
}

/// Rule 2: the assignment again; `next` is what the assignment notice asks.
pub fn reminder(a: &About<'_>, next: &str) -> String {
    format!(
        "[card {}] Automatic reminder: {} is assigned to you and is still in {}: nothing shows \
         you have started it. {next} {FROM}",
        a.id,
        a.named(),
        a.place()
    )
}

/// Rule 3: changed during the bot's turn, not moved.
pub fn ask_to_move(a: &About<'_>) -> String {
    let column = a.column_name();
    let next = match (a.column.kind, a.plan.and_then(|p| p.start.as_ref())) {
        (ColumnKind::Open, Some((start, _))) => format!(
            "If you have started it, move it to {start} with item_move; if it can't start, \
             say why in a comment on the card."
        ),
        _ if a.after_research().is_some() => format!(
            "If you have read the code, write what you found under “Code research” and move \
             it to {} with item_move. If not, say in a comment on the card what is left.",
            a.after_research().unwrap_or_default()
        ),
        _ => match a.finish() {
            Some(finish) => format!(
                "If the work is done, move it to {finish} with item_move. If it isn't, say in a \
                 comment on the card what is left or what you are waiting for."
            ),
            None => "If it isn't done, say in a comment on the card what is left or what you \
                     are waiting for."
                .to_string(),
        },
    };
    format!(
        "{} your last turn changed {}, and it is still in {column} ({}). {next} {FROM}",
        a.head(),
        a.named(),
        state_word(a.column.kind)
    )
}

/// The owner's row for a card waiting in a review column.
pub fn review_row(id: &str, title: &str) -> String {
    format!("{id} waits for your review: {}", inline(title, NAME_MAX))
}

/// The owner's row for a card nothing happened on.
pub fn quiet_row(id: &str, title: &str, column: &str, quiet: Duration) -> String {
    format!(
        "{id} is in {} and nothing has happened for {}: {}",
        inline(column, NAME_MAX),
        span(quiet),
        inline(title, NAME_MAX)
    )
}

/// The owner's row for a card whose move is refused: `detail` is
/// "<column>: <rule>".
pub fn blocked_row(id: &str, detail: &str) -> String {
    format!("{id} can't move to {detail}")
}

/// A refused move as a mark keeps it: the column's name and the rule.
pub fn refused_detail(column: &str, reason: &str) -> String {
    format!("{}: {}", inline(column, NAME_MAX), inline(reason, 160))
}

#[cfg(test)]
#[path = "text_tests.rs"]
mod tests;
