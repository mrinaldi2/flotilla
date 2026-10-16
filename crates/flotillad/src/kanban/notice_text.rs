//! What an assignee is told about its card (0.19.0, card flow): built from
//! the card's board, so the notice names the card, where it is and what
//! is expected of the bot, and says when a change is only news. Pure: the
//! caller reads the board.

use super::facts::Fact;
use super::model::{Board, BoardColumn, BoardFlow, ColumnKind, Item};

/// What happened to the card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Notice<'a> {
    /// It was assigned to its current assignee.
    Assigned,
    /// It was taken off the bot told.
    Unassigned,
    /// It moved to its current column.
    Moved,
    /// Something else changed: "edited it", "blocked it".
    Changed(&'a str),
}

/// The moves a custom board expects of the assignee, by column name.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Plan {
    /// Where work happens, and whether the daemon moves the card there
    /// when the assignee starts (`work.started`).
    pub start: Option<(String, bool)>,
    /// Where finished work goes: the next column after `start`.
    pub finish: Option<String>,
    /// Where a card that isn't ready goes back to: the last open column
    /// before `start`.
    pub back: Option<String>,
}

impl Plan {
    /// The plan of a custom board's visible columns; none for a
    /// release-gated board, whose guards decide each move.
    pub fn of(board: Option<&Board>, columns: &[BoardColumn]) -> Option<Plan> {
        let board = board.filter(|b| b.flow == BoardFlow::Custom)?;
        let mut cols: Vec<&BoardColumn> = columns
            .iter()
            .filter(|c| c.visible && c.board_id == board.id)
            .collect();
        cols.sort_by_key(|c| c.ord);
        let mapped = board
            .automations
            .get(Fact::WorkStarted.name())
            .and_then(|key| cols.iter().find(|c| &c.key == key).copied());
        let start = mapped.or_else(|| cols.iter().find(|c| c.kind == ColumnKind::Active).copied());
        let finish = start.and_then(|s| {
            cols.iter()
                .find(|c| c.ord > s.ord && c.kind != ColumnKind::Cancelled)
                .map(|c| inline(&c.name, NAME_MAX))
        });
        let back = start.and_then(|s| {
            cols.iter()
                .rev()
                .find(|c| c.ord < s.ord && c.kind == ColumnKind::Open)
                .map(|c| inline(&c.name, NAME_MAX))
        });
        Some(Plan {
            start: start.map(|s| (inline(&s.name, NAME_MAX), mapped.is_some())),
            finish,
            back,
        })
    }
}

/// A column's kind as the bot reads it.
pub fn state_word(kind: ColumnKind) -> &'static str {
    match kind {
        ColumnKind::Open => "not started",
        ColumnKind::Active => "in progress",
        ColumnKind::Done => "done",
        ColumnKind::Cancelled => "cancelled",
    }
}

/// The longest stretch of the assignee's last comment a notice quotes.
const QUOTE_MAX: usize = 160;
/// The longest title, or column name, a notice shows.
pub(crate) const NAME_MAX: usize = 120;

/// Text someone else wrote (a title, a column's name, a comment), as a
/// notice quotes it: on one line, cut to `max` characters, with no quote
/// mark that could end the quote early and no bracket that could start a
/// `[card …]` line of its own, so it can't read as the notice's text.
pub(crate) fn inline(text: &str, max: usize) -> String {
    let text: String = super::attachments::one_line(text)
        .chars()
        .map(|ch| match ch {
            '“' | '”' => '"',
            '[' => '(',
            ']' => ')',
            ch => ch,
        })
        .collect();
    match text.char_indices().nth(max) {
        Some((at, _)) => format!("{}…", &text[..at]),
        None => text,
    }
}

fn quote(text: &str) -> String {
    inline(text, QUOTE_MAX)
}

/// Everything the text needs, read by the caller.
pub struct Card<'a> {
    pub item: &'a Item,
    /// Who acted: "The owner", or a bot's name.
    pub by: &'a str,
    pub column: &'a BoardColumn,
    /// `None` on a release-gated board.
    pub plan: Option<Plan>,
    /// The assignee's latest comment on the card.
    pub last_comment: Option<&'a str>,
}

/// The notice, `[card KEY-12]` first.
pub fn text(card: &Card<'_>, notice: Notice<'_>) -> String {
    let Card {
        item, by, column, ..
    } = card;
    let id = &item.id;
    let head = format!("[card {id}]");
    let named = format!("{id} “{}”", inline(&item.title, NAME_MAX));
    let column_name = inline(&column.name, NAME_MAX);
    let place = format!("{column_name} ({})", state_word(column.kind));
    match notice {
        Notice::Assigned => format!(
            "{head} {by} assigned {named} to you. It is in {place}. {}",
            assigned_next(card)
        ),
        Notice::Unassigned => format!(
            "{head} {by} took {named} off you; it is no longer yours. Stop any work on it, \
             and don't move or edit it."
        ),
        Notice::Moved | Notice::Changed(_) => {
            let what = match notice {
                Notice::Changed(what) => what.to_string(),
                _ => format!("moved it to {column_name}"),
            };
            let mut out = format!(
                "{head} {by} changed {named}, which is assigned to you: {what}. It is now in \
                 {place}. {}",
                changed_next(column.kind)
            );
            if let Some(comment) = card.last_comment {
                out.push_str(&format!(" Your last comment on it: “{}”", quote(comment)));
            }
            out
        }
    }
}

/// What an assignment asks of the bot, on this board.
pub(crate) fn assigned_next(card: &Card<'_>) -> String {
    let id = &card.item.id;
    let ask = format!(
        "If anything is unclear or it isn't ready, ask the owner with message_owner (asks: \
         true, item: {id})"
    );
    let Some(plan) = &card.plan else {
        return format!(
            "Read it with item_get. This board's release-gated rules decide each move: \
             item_move_check shows which moves are yours and what each needs. {ask}."
        );
    };
    let finish = match &plan.finish {
        Some(finish) => format!(
            " When the work is done, comment the result on the card, link or attach what you \
             made, and move it to {finish} with item_move."
        ),
        None => " When the work is done, comment the result on the card and link or attach \
                 what you made."
            .to_string(),
    };
    let back = match &plan.back {
        Some(back) => format!("{ask}, after moving it back to {back}."),
        None => format!("{ask}."),
    };
    match (card.column.kind, &plan.start) {
        (ColumnKind::Open, Some((start, true))) => {
            format!("Start now: read it with item_get, which moves it to {start}.{finish} {back}")
        }
        (ColumnKind::Open, Some((start, false))) => format!(
            "Start now: read it with item_get and move it to {start} with item_move.{finish} \
             {back}"
        ),
        (ColumnKind::Active, _) | (ColumnKind::Open, None) => {
            format!("Read it with item_get and carry on from where it is.{finish} {back}")
        }
        (ColumnKind::Done | ColumnKind::Cancelled, _) => format!(
            "There may be nothing left to do: read it with item_get, and {}.",
            ask.replacen("If anything", "if anything", 1)
        ),
    }
}

/// What a change asks of the bot: nothing new. A notice never starts work.
fn changed_next(kind: ColumnKind) -> &'static str {
    match kind {
        ColumnKind::Done => "It is done: there is nothing to do.",
        ColumnKind::Cancelled => "It is cancelled: stop any work on it.",
        ColumnKind::Open | ColumnKind::Active => {
            "This is a notice, not new work: if your work on it is finished, don't redo it; \
             otherwise carry on. Read it again with item_get before you change it."
        }
    }
}

#[cfg(test)]
mod tests;
