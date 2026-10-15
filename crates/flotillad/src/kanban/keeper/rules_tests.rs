//! The keeper's rules on their own: each fires once, holds off before its
//! time and while the bot works, resets when the card changes, and counts
//! from the daemon's start at the earliest.

use chrono::{DateTime, Duration, Utc};

use super::*;
use crate::kanban::model::ColumnCategory;

fn t0() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-10-14T08:00:00Z")
        .unwrap()
        .with_timezone(&Utc)
}

fn h(n: i64) -> Duration {
    Duration::hours(n)
}

fn limits() -> Limits {
    Limits {
        nudge_after: h(2),
        needs_you_after: h(6),
        not_started_after: Duration::minutes(30),
        blocked_after: h(1),
    }
}

fn card(column: &str) -> KeeperCard {
    KeeperCard {
        id: "RB-12".into(),
        board_id: "b".into(),
        title: "Buttons".into(),
        column_key: column.into(),
        category: ColumnCategory::Doing,
        assignee: Some("bot-1".into()),
        entered_at: t0(),
        last_event_at: Some(t0()),
        assigned_at: Some(t0()),
        assignee_acted_at: None,
    }
}

fn facts(card: &KeeperCard, role: Role) -> Facts<'_> {
    Facts {
        card,
        custom: true,
        role,
        ages_after: None,
        reachable: true,
        busy: false,
        last_turn: None,
        touched_at: None,
        refused: None,
    }
}

/// A clock `hours` after t0, for a daemon that started at t0.
fn at(hours: i64) -> Clock {
    Clock {
        now: t0() + h(hours),
        boot: t0(),
    }
}

/// The marks a plan leaves, applied over `marks`.
fn after(marks: &[KeeperMark], plan: &Plan) -> Vec<KeeperMark> {
    let mut out: Vec<KeeperMark> = marks.to_vec();
    for act in &plan.acts {
        match act {
            Act::Say(_, m) | Act::Set(m) => {
                out.retain(|o| o.rule != m.rule);
                out.push(m.clone());
            }
            Act::Clear(rule) => out.retain(|o| o.rule != rule.name()),
            Act::Raised(_) => {}
        }
    }
    out
}

fn says(plan: &Plan) -> Vec<&Say> {
    plan.acts
        .iter()
        .filter_map(|a| match a {
            Act::Say(say, _) => Some(say),
            _ => None,
        })
        .collect()
}

fn raised(plan: &Plan) -> bool {
    plan.acts.iter().any(|a| matches!(a, Act::Raised(_)))
}

#[test]
fn a_quiet_card_is_nudged_once_then_flagged_once() {
    let c = card("doing");
    let f = facts(&c, Role::Work);
    assert!(judge(&f, &[], at(1), &limits()).is_empty(), "too soon");
    let nudge = judge(&f, &[], at(2), &limits());
    assert_eq!(says(&nudge), [&Say::Nudge(h(2))]);
    let marks = after(&[], &nudge);
    // Not again, and not flagged before the second threshold.
    assert!(judge(&f, &marks, at(3), &limits()).is_empty());
    assert!(judge(&f, &marks, at(5), &limits()).is_empty());
    let flag = judge(&f, &marks, at(6), &limits());
    assert!(raised(&flag) && flag.publish, "{flag:?}");
    assert!(says(&flag).is_empty());
    let marks = after(&marks, &flag);
    assert!(judge(&f, &marks, at(30), &limits()).is_empty(), "once");
}

#[test]
fn anything_on_the_card_after_the_nudge_starts_it_over() {
    let mut c = card("doing");
    let nudge = judge(&facts(&c, Role::Work), &[], at(2), &limits());
    let flag_marks = after(&[], &nudge);
    let flag = judge(&facts(&c, Role::Work), &flag_marks, at(6), &limits());
    let marks = after(&flag_marks, &flag);
    c.last_event_at = Some(t0() + h(7));
    let reset = judge(&facts(&c, Role::Work), &marks, at(8), &limits());
    assert!(reset.publish, "the stale flag goes: {reset:?}");
    let marks = after(&marks, &reset);
    assert!(marks.is_empty(), "{marks:?}");
    // Quiet again for the full time: a new nudge.
    assert!(judge(&facts(&c, Role::Work), &marks, at(8), &limits()).is_empty());
    let again = judge(&facts(&c, Role::Work), &marks, at(9), &limits());
    assert_eq!(says(&again).len(), 1);
}

#[test]
fn a_bot_in_a_turn_or_a_recent_turn_is_not_idle() {
    let c = card("doing");
    let mut f = facts(&c, Role::Work);
    f.busy = true;
    assert!(judge(&f, &[], at(10), &limits()).is_empty());
    f.busy = false;
    f.last_turn = Some(t0() + h(9));
    assert!(judge(&f, &[], at(10), &limits()).is_empty());
    assert_eq!(says(&judge(&f, &[], at(11), &limits())).len(), 1);
}

#[test]
fn timers_count_from_the_daemons_start() {
    let mut c = card("doing");
    c.entered_at = t0() - h(48);
    c.last_event_at = Some(t0() - h(48));
    let f = facts(&c, Role::Work);
    // Down for two days, up for an hour: nothing yet.
    assert!(judge(&f, &[], at(1), &limits()).is_empty());
    assert_eq!(says(&judge(&f, &[], at(2), &limits())).len(), 1);
}

#[test]
fn a_release_gated_card_or_an_unreachable_bot_gets_only_the_row() {
    let c = card("doing");
    for f in [
        Facts {
            custom: false,
            ..facts(&c, Role::Work)
        },
        Facts {
            reachable: false,
            ..facts(&c, Role::Work)
        },
    ] {
        assert!(judge(&f, &[], at(5), &limits()).is_empty());
        let row = judge(&f, &[], at(6), &limits());
        assert!(raised(&row) && says(&row).is_empty(), "{row:?}");
    }
}

#[test]
fn a_move_or_a_new_assignee_drops_the_marks() {
    let c = card("doing");
    let marks = after(&[], &judge(&facts(&c, Role::Work), &[], at(2), &limits()));
    let moved = card("review");
    let plan = judge(&facts(&moved, Role::Review), &marks, at(3), &limits());
    assert_eq!(plan.acts, [Act::Clear(Rule::SilentNudged)]);
    let mut other = card("doing");
    other.assignee = Some("bot-2".into());
    let plan = judge(&facts(&other, Role::Work), &marks, at(3), &limits());
    assert!(plan.acts.contains(&Act::Clear(Rule::SilentNudged)));
}

#[test]
fn an_untouched_assignment_is_told_again_once_per_assignment() {
    let mut c = card("todo");
    c.category = ColumnCategory::Inbox;
    let f = facts(&c, Role::Open);
    let early = Clock {
        now: t0() + Duration::minutes(29),
        boot: t0(),
    };
    assert!(judge(&f, &[], early, &limits()).is_empty());
    let late = Clock {
        now: t0() + Duration::minutes(30),
        boot: t0(),
    };
    let told = judge(&f, &[], late, &limits());
    assert_eq!(says(&told), [&Say::Reminder]);
    let marks = after(&[], &told);
    assert!(judge(&f, &marks, at(5), &limits()).is_empty(), "once");
    // Touched (read or changed) since the assignment: nothing to remind.
    let touched = Facts {
        touched_at: Some(t0() + Duration::minutes(1)),
        ..facts(&c, Role::Open)
    };
    assert!(judge(&touched, &[], at(5), &limits()).is_empty());
    // Assigned again: a new reminder may follow.
    c.assigned_at = Some(t0() + h(4));
    let again = judge(&facts(&c, Role::Open), &marks, at(5), &limits());
    assert_eq!(says(&again), [&Say::Reminder]);
    // Never on a release-gated board.
    let gated = Facts {
        custom: false,
        ..facts(&c, Role::Open)
    };
    assert!(judge(&gated, &[], at(9), &limits()).is_empty());
}

#[test]
fn staleness_is_marked_and_pushed_only_when_it_happens_now() {
    let mut c = card("doing");
    c.last_event_at = Some(t0() + h(100));
    let f = Facts {
        ages_after: Some(h(24)),
        busy: true,
        ..facts(&c, Role::Work)
    };
    assert!(judge(&f, &[], at(23), &limits()).is_empty());
    let stale = judge(&f, &[], at(24), &limits());
    assert!(stale.publish, "{stale:?}");
    let marks = after(&[], &stale);
    assert!(judge(&f, &marks, at(30), &limits()).is_empty(), "once");
    // Stale before the daemon started: marked, not pushed.
    let mut old = card("doing");
    old.entered_at = t0() - h(30);
    let quiet = judge(
        &Facts {
            ages_after: Some(h(24)),
            busy: true,
            ..facts(&old, Role::Work)
        },
        &[],
        at(0),
        &limits(),
    );
    assert!(!quiet.publish && !quiet.acts.is_empty(), "{quiet:?}");
    // No longer ageing (the board's setting grew): cleared and pushed.
    let fresh = Facts {
        ages_after: Some(h(48)),
        busy: true,
        ..facts(&c, Role::Work)
    };
    let cleared = judge(&fresh, &marks, at(30), &limits());
    assert_eq!(cleared.acts, [Act::Clear(Rule::Stale)]);
    assert!(cleared.publish);
}

#[test]
fn a_refused_move_raises_its_row_after_the_threshold() {
    let c = card("doing");
    let f = Facts {
        refused: Some("Review: No pr is linked.".into()),
        busy: true,
        ..facts(&c, Role::Work)
    };
    let seen = judge(&f, &[], at(0), &limits());
    assert!(!seen.publish && !raised(&seen), "{seen:?}");
    let marks = after(&[], &seen);
    assert!(judge(
        &f,
        &marks,
        Clock {
            now: t0() + Duration::minutes(59),
            boot: t0()
        },
        &limits()
    )
    .is_empty());
    let row = judge(&f, &marks, at(1), &limits());
    assert!(raised(&row) && row.publish, "{row:?}");
    let marks = after(&marks, &row);
    assert!(judge(&f, &marks, at(5), &limits()).is_empty(), "once");
    // The move would pass now: the row goes.
    let passes = Facts {
        refused: None,
        busy: true,
        ..facts(&c, Role::Work)
    };
    let gone = judge(&passes, &marks, at(6), &limits());
    assert_eq!(gone.acts, [Act::Clear(Rule::Blocked)]);
    assert!(gone.publish);
}

#[test]
fn a_turn_that_changed_a_card_without_moving_it_asks_once() {
    let c = card("doing");
    let work = TurnWork {
        item_id: c.id.clone(),
        last_at: t0() + h(1),
        moved: false,
    };
    let now = t0() + h(1);
    let f = facts(&c, Role::Work);
    let Some(Act::Say(Say::AskToMove, mark)) = after_turn(&f, &work, None, now) else {
        panic!("an ask");
    };
    assert!(after_turn(&f, &work, Some(&mark), now).is_none(), "once");
    let moved = TurnWork {
        moved: true,
        ..work.clone()
    };
    assert!(after_turn(&f, &moved, None, now).is_none());
    for f in [
        facts(&c, Role::Review),
        Facts {
            custom: false,
            ..facts(&c, Role::Work)
        },
    ] {
        assert!(after_turn(&f, &work, None, now).is_none());
    }
    // Moved since it was asked: the old mark no longer holds.
    let elsewhere = KeeperMark {
        column_key: "review".into(),
        ..mark
    };
    assert!(after_turn(&f, &work, Some(&elsewhere), now).is_some());
}
