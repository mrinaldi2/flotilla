//! The keeper's rules, pure: given what a card is like now and what the
//! keeper already did about it, what to do. Each rule acts at most once
//! per card, condition and assignee (or column) until the card changes,
//! and every timer counts from the daemon's start at the earliest.

use chrono::{DateTime, Duration, Utc};

use super::config::Limits;
use crate::db::{KeeperCard, KeeperMark, TurnWork};

/// What a mark is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rule {
    /// The assignee was asked about its quiet card.
    SilentNudged,
    /// Still quiet: the card is stale and the owner has a row.
    SilentStale,
    /// The assignment was told again.
    NotStarted,
    /// After a turn that changed it, the bot was asked to move it.
    NotMoved,
    /// A refused move, seen since `since`; a row once `fired_at` is set.
    Blocked,
    /// The card has sat in its column past the board's `stale_after_hours`.
    Stale,
}

impl Rule {
    pub fn name(self) -> &'static str {
        match self {
            Rule::SilentNudged => "silent_nudged",
            Rule::SilentStale => "silent_stale",
            Rule::NotStarted => "not_started",
            Rule::NotMoved => "not_moved",
            Rule::Blocked => "blocked",
            Rule::Stale => "stale",
        }
    }
}

/// What the card's column means for the keeper.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// Not started: To do.
    Open,
    /// Where work happens: Doing.
    Work,
    /// A custom board's active column after its work column: Review, where
    /// the card waits for someone else.
    Review,
    /// Done, cancelled, or a release-gated column the keeper leaves alone.
    Other,
}

/// What the keeper knows about one card now.
#[derive(Debug, Clone)]
pub struct Facts<'a> {
    pub card: &'a KeeperCard,
    /// A custom board; a release-gated one gets only the staleness and the
    /// Needs-you rows.
    pub custom: bool,
    pub role: Role,
    /// Its column is one a card goes stale in, and how long that takes.
    pub ages_after: Option<Duration>,
    /// The assignee is a live bot a message can reach.
    pub reachable: bool,
    /// The assignee is in a turn now.
    pub busy: bool,
    pub last_turn: Option<DateTime<Utc>>,
    /// The assignee's latest change to the card, or read of it.
    pub touched_at: Option<DateTime<Utc>>,
    /// A move of it that was refused and still would be: "Review: why".
    pub refused: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct Clock {
    pub now: DateTime<Utc>,
    pub boot: DateTime<Utc>,
}

/// A message to the card's assignee.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Say {
    /// Quiet in its work column for this long.
    Nudge(Duration),
    /// The assignment, again.
    Reminder,
    /// Changed in a turn and not moved.
    AskToMove,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Act {
    /// Send it, then write the mark; if it can't be sent, neither happens.
    Say(Say, KeeperMark),
    Set(KeeperMark),
    Clear(Rule),
    /// A Needs-you row came up, for the owner's notices.
    Raised(Rule),
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Plan {
    pub acts: Vec<Act>,
    /// The card as the board shows it changed: push it.
    pub publish: bool,
}

impl Plan {
    pub fn is_empty(&self) -> bool {
        self.acts.is_empty() && !self.publish
    }
}

/// Whether a mark is still true of the card: same column and, but for
/// `stale`, the same assignee.
pub fn holds(m: &KeeperMark, card: &KeeperCard) -> bool {
    m.column_key == card.column_key
        && (m.rule == Rule::Stale.name() || m.assignee == card.assignee.as_deref().unwrap_or(""))
}

fn new_mark(card: &KeeperCard, rule: Rule, since: DateTime<Utc>) -> KeeperMark {
    KeeperMark {
        item_id: card.id.clone(),
        rule: rule.name().to_string(),
        assignee: if rule == Rule::Stale {
            String::new()
        } else {
            card.assignee.clone().unwrap_or_default()
        },
        column_key: card.column_key.clone(),
        since,
        fired_at: None,
        detail: String::new(),
    }
}

fn fired(card: &KeeperCard, rule: Rule, since: DateTime<Utc>, now: DateTime<Utc>) -> KeeperMark {
    KeeperMark {
        fired_at: Some(now),
        ..new_mark(card, rule, since)
    }
}

/// The later of the times given.
fn latest(times: &[Option<DateTime<Utc>>]) -> Option<DateTime<Utc>> {
    times.iter().flatten().max().copied()
}

/// What to do about the card on a pass.
pub fn judge(f: &Facts<'_>, marks: &[KeeperMark], clock: Clock, limits: &Limits) -> Plan {
    let card = f.card;
    let mut plan = Plan::default();
    let mut live: Vec<&KeeperMark> = Vec::new();
    for m in marks.iter().filter(|m| m.item_id == card.id) {
        if holds(m, card) {
            live.push(m);
        } else if let Some(rule) = rule_of(&m.rule) {
            // The card moved or changed hands: its board read already
            // stopped showing it.
            plan.acts.push(Act::Clear(rule));
        }
    }
    let mark = |rule: Rule| live.iter().copied().find(|m| m.rule == rule.name());
    stale(f, mark(Rule::Stale), clock, &mut plan);
    silent(
        f,
        (mark(Rule::SilentNudged), mark(Rule::SilentStale)),
        clock,
        limits,
        &mut plan,
    );
    not_started(f, mark(Rule::NotStarted), clock, limits, &mut plan);
    blocked(f, mark(Rule::Blocked), clock, limits, &mut plan);
    plan
}

fn rule_of(name: &str) -> Option<Rule> {
    [
        Rule::SilentNudged,
        Rule::SilentStale,
        Rule::NotStarted,
        Rule::NotMoved,
        Rule::Blocked,
        Rule::Stale,
    ]
    .into_iter()
    .find(|r| r.name() == name)
}

/// Rule 6: past the board's `stale_after_hours` in a column that ages.
/// A card that went stale while the daemon was down is marked quietly: any
/// board read since shows it, so only one that goes stale now is pushed.
fn stale(f: &Facts<'_>, mark: Option<&KeeperMark>, clock: Clock, plan: &mut Plan) {
    let due = f.ages_after.map(|after| f.card.entered_at + after);
    match (due.filter(|due| *due <= clock.now), mark) {
        (Some(due), None) => {
            plan.acts
                .push(Act::Set(fired(f.card, Rule::Stale, due, clock.now)));
            plan.publish |= due > clock.boot;
        }
        (None, Some(_)) => {
            plan.acts.push(Act::Clear(Rule::Stale));
            plan.publish = true;
        }
        _ => {}
    }
}

/// Rule 1: a card in its work column whose assignee had no turn and on
/// which nothing happened. A custom board's assignee is asked once; still
/// quiet later, the card is stale and the owner has a row. A release-gated
/// board, or an assignee no message reaches, goes straight to the row.
/// Anything happening on the card after either clears both.
fn silent(
    f: &Facts<'_>,
    (mut nudged, mut flagged): (Option<&KeeperMark>, Option<&KeeperMark>),
    clock: Clock,
    limits: &Limits,
    plan: &mut Plan,
) {
    let card = f.card;
    if f.role != Role::Work || card.assignee.is_none() {
        return;
    }
    let marked_at = latest(&[
        nudged.and_then(|m| m.fired_at),
        flagged.and_then(|m| m.fired_at),
    ]);
    if let (Some(marked), Some(event)) = (marked_at, card.last_event_at) {
        if event > marked {
            if nudged.take().is_some() {
                plan.acts.push(Act::Clear(Rule::SilentNudged));
            }
            if flagged.take().is_some() {
                plan.acts.push(Act::Clear(Rule::SilentStale));
                plan.publish = true;
            }
        }
    }
    if f.busy || flagged.is_some() {
        return;
    }
    let quiet_since = latest(&[
        Some(card.entered_at),
        card.last_event_at,
        f.last_turn,
        Some(clock.boot),
    ])
    .unwrap_or(clock.boot);
    let quiet = clock.now - quiet_since;
    let raise = |plan: &mut Plan, since: DateTime<Utc>| {
        plan.acts
            .push(Act::Set(fired(card, Rule::SilentStale, since, clock.now)));
        plan.acts.push(Act::Raised(Rule::SilentStale));
        plan.publish = true;
    };
    if !(f.custom && f.reachable) {
        if quiet >= limits.needs_you_after {
            raise(plan, quiet_since);
        }
        return;
    }
    match nudged {
        None if quiet >= limits.nudge_after => plan.acts.push(Act::Say(
            Say::Nudge(quiet),
            fired(card, Rule::SilentNudged, quiet_since, clock.now),
        )),
        Some(n) => {
            let asked = n.fired_at.unwrap_or(n.since).max(clock.boot);
            if clock.now - asked >= limits.needs_you_after - limits.nudge_after {
                raise(plan, n.since);
            }
        }
        None => {}
    }
}

/// Rule 2: an assigned card in an open column its assignee hasn't touched
/// since the assignment: the assignment is told again, once per assignment.
fn not_started(
    f: &Facts<'_>,
    mark: Option<&KeeperMark>,
    clock: Clock,
    limits: &Limits,
    plan: &mut Plan,
) {
    let card = f.card;
    let Some(assigned) = card.assigned_at else {
        return;
    };
    if !f.custom || f.role != Role::Open || !f.reachable || card.assignee.is_none() {
        return;
    }
    let touched = f.touched_at.is_some_and(|t| t >= assigned);
    let told = mark.is_some_and(|m| m.since == assigned);
    if touched || told || clock.now - assigned.max(clock.boot) < limits.not_started_after {
        return;
    }
    plan.acts.push(Act::Say(
        Say::Reminder,
        fired(card, Rule::NotStarted, assigned, clock.now),
    ));
}

/// Rule 5: a refused move that would still be refused. Seen first, then,
/// after the threshold, a row naming the rule; gone once it would pass.
fn blocked(
    f: &Facts<'_>,
    mark: Option<&KeeperMark>,
    clock: Clock,
    limits: &Limits,
    plan: &mut Plan,
) {
    if f.role == Role::Other {
        return;
    }
    match (&f.refused, mark) {
        (Some(detail), Some(m)) if m.detail == *detail => {
            if m.fired_at.is_none() && clock.now - m.since.max(clock.boot) >= limits.blocked_after {
                plan.acts.push(Act::Set(KeeperMark {
                    fired_at: Some(clock.now),
                    ..m.clone()
                }));
                plan.acts.push(Act::Raised(Rule::Blocked));
                plan.publish = true;
            }
        }
        (Some(detail), m) => {
            plan.acts.push(Act::Set(KeeperMark {
                detail: detail.clone(),
                ..new_mark(f.card, Rule::Blocked, clock.now)
            }));
            plan.publish |= m.is_some_and(|m| m.fired_at.is_some());
        }
        (None, Some(m)) => {
            plan.acts.push(Act::Clear(Rule::Blocked));
            plan.publish |= m.fired_at.is_some();
        }
        (None, None) => {}
    }
}

/// Rule 3, after a turn of the assignee that changed the card and moved
/// no card: ask it once to move it on, or to say why not.
pub fn after_turn(
    f: &Facts<'_>,
    work: &TurnWork,
    mark: Option<&KeeperMark>,
    now: DateTime<Utc>,
) -> Option<Act> {
    let card = f.card;
    let open_or_work = matches!(f.role, Role::Open | Role::Work);
    let asked = mark.is_some_and(|m| holds(m, card));
    if !f.custom || !f.reachable || !open_or_work || work.moved || asked {
        return None;
    }
    Some(Act::Say(
        Say::AskToMove,
        fired(card, Rule::NotMoved, work.last_at, now),
    ))
}

#[cfg(test)]
#[path = "rules_tests.rs"]
mod tests;
