//! How a custom board decides a move (0.19.0 K3): the target column's own
//! movers, allowed sources and entry rules, from a fixed library of
//! deterministic checks, then its WIP limit. Like `guards`, nothing here
//! reads storage; a refusal lists every unmet rule with its fix.

use serde::{Deserialize, Serialize};

use super::guards::{over_limit, unmet, wip_unmet, Context, Move, Who};
use super::model::{BoardColumn, ColumnKind, Item, LinkKind, Size, Unmet};

/// Who may move an item into a column; any one of a column's movers will do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mover {
    AnyMember,
    Assignee,
    Lead,
    Owner,
    /// Only automations (facts) move items in.
    System,
    /// Bots holding any of these board roles (`dev`, `tester`, …).
    Roles(Vec<String>),
}

/// One deterministic check a column makes before an item may enter it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "rule", rename_all = "snake_case")]
pub enum EntryRule {
    AssigneeSet,
    /// These description sections (template headings) have text.
    FieldsFilled {
        sections: Vec<String>,
    },
    AcPresent,
    AcAllChecked,
    SizeAtMost {
        size: Size,
    },
    BlockersDone,
    NotBlocked,
    /// A link of this kind (`pr`, `branch`, `artifact`, `task`, …).
    Link {
        kind: String,
    },
    /// The mover is neither the assignee, nor a bot that ever was, nor
    /// whoever linked its branch or PR.
    Independent,
    Reason,
    /// A task or bug has its code research recorded (0.21.0,
    /// `start.research`); other types pass.
    ResearchRecorded,
    /// A check another module registers (`release.in_open_package`).
    Predicate {
        name: String,
    },
}

/// A custom board's rules for entering one column.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ColumnPolicy {
    /// Empty: any member.
    pub movers: Vec<Mover>,
    pub rules: Vec<EntryRule>,
    /// Column keys an item may come from; empty: any.
    pub allowed_from: Vec<String>,
}

/// The checks other modules register: named facts about an item that a
/// rule can require. Evaluated from the move's context, so still pure.
pub fn predicate(name: &str, ctx: &Context) -> Option<bool> {
    match name {
        "release.in_open_package" => Some(ctx.open_package.is_some()),
        "code.linked" => Some(ctx.has_link(&[LinkKind::Branch, LinkKind::Pr])),
        _ => None,
    }
}

/// Moving to an earlier column is a return: never refused for WIP, and the
/// item is placed first.
pub fn is_return(from: &BoardColumn, to: &BoardColumn) -> bool {
    to.ord < from.ord
}

/// Everything that stands between `item` (in `from`) and `mv.to` on a
/// custom board; empty means allowed.
pub fn evaluate(
    item: &Item,
    from: &BoardColumn,
    policy: &ColumnPolicy,
    mv: &Move<'_>,
    who: &Who,
    ctx: &Context,
) -> Vec<Unmet> {
    let to = mv.to;
    if to.key == item.column_key {
        return vec![unmet(
            "move.same_column",
            "It is already in this column.",
            None,
        )];
    }
    if *who == Who::Daemon {
        return Vec::new();
    }
    let mut out = Vec::new();
    if !policy.allowed_from.is_empty() && !policy.allowed_from.contains(&from.key) {
        out.push(unmet(
            "move.from",
            format!(
                "Items come to {} only from {}.",
                to.name,
                policy.allowed_from.join(", ")
            ),
            None,
        ));
    }
    if let Some(who_may) = refused(&policy.movers, item, who) {
        out.push(unmet(
            "role.not_allowed",
            format!("Only {who_may} can move it to {}.", to.name),
            None,
        ));
    }
    for rule in &policy.rules {
        out.extend(check(rule, item, mv, who, ctx));
    }
    if !is_return(from, to) && over_limit(item, to, ctx).is_some() {
        out.extend(wip_unmet(item, mv, who, ctx));
    }
    out
}

/// Who may move the item in, when `who` may not.
fn refused(movers: &[Mover], item: &Item, who: &Who) -> Option<String> {
    if *who == Who::Owner {
        return movers
            .contains(&Mover::System)
            .then(|| "automations".to_string());
    }
    if movers.is_empty() {
        return None;
    }
    let ok = movers.iter().any(|m| match m {
        Mover::AnyMember => true,
        Mover::Assignee => who.is(item.assignee.as_deref()),
        Mover::Lead => who.leads(),
        Mover::Owner | Mover::System => false,
        Mover::Roles(roles) => roles
            .iter()
            .any(|r| super::model::Role::parse(r).is_some_and(|r| who.has(r))),
    });
    (!ok).then(|| describe(movers))
}

fn describe(movers: &[Mover]) -> String {
    if movers.contains(&Mover::System) {
        return "automations".to_string();
    }
    let words: Vec<String> = movers
        .iter()
        .map(|m| match m {
            Mover::AnyMember => "a member".to_string(),
            Mover::Assignee => "the assignee".to_string(),
            Mover::Lead => "the lead".to_string(),
            Mover::Owner | Mover::System => "the owner".to_string(),
            Mover::Roles(roles) => roles.join(" or "),
        })
        .chain(std::iter::once("the owner".to_string()))
        .collect();
    words.join(", ")
}

fn check(rule: &EntryRule, item: &Item, mv: &Move<'_>, who: &Who, ctx: &Context) -> Vec<Unmet> {
    let fail = |code: &str, text: String, fix: Option<&str>| vec![unmet(code, text, fix)];
    match rule {
        EntryRule::AssigneeSet if item.assignee.is_none() => fail(
            "rule.assignee_set",
            "Nobody is assigned.".into(),
            Some("Assign it first."),
        ),
        EntryRule::FieldsFilled { sections } => sections
            .iter()
            .filter(|s| !super::guards::section_filled(&item.description, s))
            .map(|s| unmet("rule.fields_filled", format!("Fill in “{s}”."), None))
            .collect(),
        EntryRule::AcPresent if item.acceptance_criteria.is_empty() => fail(
            "rule.ac_present",
            "Add at least one acceptance criterion.".into(),
            None,
        ),
        EntryRule::AcAllChecked => {
            let open = item
                .acceptance_criteria
                .iter()
                .filter(|a| !a.checked)
                .count();
            if open == 0 {
                Vec::new()
            } else {
                fail(
                    "rule.ac_all_checked",
                    format!("{open} acceptance criteria are not checked."),
                    None,
                )
            }
        }
        EntryRule::SizeAtMost { size } => match item.size {
            None => fail("rule.size_at_most", "Size it.".into(), None),
            Some(s) if s > *size => fail(
                "rule.size_at_most",
                format!("Size {} is too big here.", s.as_str()),
                Some("Split it."),
            ),
            Some(_) => Vec::new(),
        },
        EntryRule::BlockersDone => ctx
            .blockers
            .iter()
            .filter(|b| b.category.kind() != ColumnKind::Done)
            .map(|b| {
                unmet(
                    "rule.blockers_done",
                    format!("{} blocks it and isn't done.", b.id),
                    None,
                )
            })
            .collect(),
        EntryRule::NotBlocked => match &item.blocked {
            Some(b) => fail(
                "rule.not_blocked",
                format!("It is blocked: {}.", b.reason),
                Some("Unblock it first."),
            ),
            None => Vec::new(),
        },
        EntryRule::Link { kind } => match LinkKind::parse(kind) {
            None => fail("rule.link", format!("“{kind}” is not a link kind."), None),
            Some(k) if !ctx.has_link(&[k]) => fail(
                "rule.link",
                format!("No {kind} is linked."),
                Some("Link it first."),
            ),
            Some(_) => Vec::new(),
        },
        EntryRule::Independent => independent(item, who, ctx),
        EntryRule::ResearchRecorded => super::guards::unmet_research(item, ctx)
            .into_iter()
            .collect(),
        EntryRule::Reason if !mv.reason.is_some_and(|r| !r.trim().is_empty()) => {
            fail("reason.required", "Give a reason.".into(), None)
        }
        EntryRule::Predicate { name } => match predicate(name, ctx) {
            Some(true) => Vec::new(),
            Some(false) => fail("rule.predicate", format!("“{name}” doesn't hold."), None),
            None => fail(
                "rule.predicate",
                format!("“{name}” is not a known rule."),
                None,
            ),
        },
        _ => Vec::new(),
    }
}

fn independent(item: &Item, who: &Who, ctx: &Context) -> Vec<Unmet> {
    let Who::Bot { id, .. } = who else {
        return Vec::new();
    };
    let author = format!("bot:{id}");
    let linked = ctx
        .links
        .iter()
        .any(|l| matches!(l.kind, LinkKind::Branch | LinkKind::Pr) && l.created_by == author);
    // A bot that held the card did its work, even once it hands it on.
    let held = ctx.past_assignees.iter().any(|b| who.is(Some(b)));
    if who.is(item.assignee.as_deref()) || held || linked {
        vec![unmet(
            "rule.independent",
            "The mover can't be its assignee, now or before, or an author of its branch.",
            Some("Ask someone else."),
        )]
    } else {
        Vec::new()
    }
}

pub mod tokens;

#[cfg(test)]
mod first_board_tests;
#[cfg(test)]
mod tests;
