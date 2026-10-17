//! Movers and rules as the short tokens the wire, bots and the apps use:
//! `lead`, `roles:dev,tester`, `link:pr`, `size_at_most:M`,
//! `fields_filled:Goal;Steps`. Parsing refuses anything it doesn't know,
//! so a typo can't switch a rule off.

use super::{EntryRule, Mover};
use crate::kanban::model::{LinkKind, Size};

impl Mover {
    pub fn token(&self) -> String {
        match self {
            Mover::AnyMember => "any_member".into(),
            Mover::Assignee => "assignee".into(),
            Mover::Lead => "lead".into(),
            Mover::Owner => "owner".into(),
            Mover::System => "system".into(),
            Mover::Roles(roles) => format!("roles:{}", roles.join(",")),
        }
    }

    pub fn parse(token: &str) -> Result<Mover, String> {
        let (head, arg) = split(token);
        Ok(match (head, arg) {
            ("any_member", None) => Mover::AnyMember,
            ("assignee", None) => Mover::Assignee,
            ("lead", None) => Mover::Lead,
            ("owner", None) => Mover::Owner,
            ("system", None) => Mover::System,
            ("roles", Some(list)) => {
                let roles: Vec<String> = list.split(',').map(|r| r.trim().to_string()).collect();
                if let Some(bad) = roles
                    .iter()
                    .find(|r| crate::kanban::model::Role::parse(r).is_none())
                {
                    return Err(format!("“{bad}” is not a board role"));
                }
                Mover::Roles(roles)
            }
            _ => return Err(format!("“{token}” is not a mover")),
        })
    }
}

impl EntryRule {
    pub fn token(&self) -> String {
        match self {
            EntryRule::AssigneeSet => "assignee_set".into(),
            EntryRule::FieldsFilled { sections } => format!("fields_filled:{}", sections.join(";")),
            EntryRule::AcPresent => "ac_present".into(),
            EntryRule::AcAllChecked => "ac_all_checked".into(),
            EntryRule::SizeAtMost { size } => format!("size_at_most:{}", size.as_str()),
            EntryRule::BlockersDone => "blockers_done".into(),
            EntryRule::NotBlocked => "not_blocked".into(),
            EntryRule::Link { kind } => format!("link:{kind}"),
            EntryRule::Independent => "independent".into(),
            EntryRule::Reason => "reason".into(),
            EntryRule::ResearchRecorded => "research_recorded".into(),
            EntryRule::Predicate { name } => format!("predicate:{name}"),
        }
    }

    pub fn parse(token: &str) -> Result<EntryRule, String> {
        let (head, arg) = split(token);
        Ok(match (head, arg) {
            ("assignee_set", None) => EntryRule::AssigneeSet,
            ("fields_filled", Some(list)) => EntryRule::FieldsFilled {
                sections: list
                    .split(';')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .collect(),
            },
            ("ac_present", None) => EntryRule::AcPresent,
            ("ac_all_checked", None) => EntryRule::AcAllChecked,
            ("size_at_most", Some(size)) => EntryRule::SizeAtMost {
                size: Size::parse(size).ok_or_else(|| format!("“{size}” is not a size"))?,
            },
            ("blockers_done", None) => EntryRule::BlockersDone,
            ("not_blocked", None) => EntryRule::NotBlocked,
            ("link", Some(kind)) => {
                LinkKind::parse(kind).ok_or_else(|| format!("“{kind}” is not a link kind"))?;
                EntryRule::Link { kind: kind.into() }
            }
            ("independent", None) => EntryRule::Independent,
            ("reason", None) => EntryRule::Reason,
            ("research_recorded", None) => EntryRule::ResearchRecorded,
            ("predicate", Some(name)) => {
                if super::predicate(name, &Default::default()).is_none() {
                    return Err(format!("“{name}” is not a known rule"));
                }
                EntryRule::Predicate { name: name.into() }
            }
            _ => return Err(format!("“{token}” is not a rule")),
        })
    }
}

fn split(token: &str) -> (&str, Option<&str>) {
    match token.trim().split_once(':') {
        Some((head, arg)) => (head.trim(), Some(arg.trim())),
        None => (token.trim(), None),
    }
}

/// Every token parsed, or the first one that isn't valid.
pub fn movers(tokens: &[String]) -> Result<Vec<Mover>, String> {
    tokens.iter().map(|t| Mover::parse(t)).collect()
}

pub fn rules(tokens: &[String]) -> Result<Vec<EntryRule>, String> {
    tokens.iter().map(|t| EntryRule::parse(t)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_token_parses_back_to_itself() {
        for token in [
            "any_member",
            "assignee",
            "lead",
            "owner",
            "system",
            "roles:dev,tester",
        ] {
            assert_eq!(Mover::parse(token).expect(token).token(), token);
        }
        for token in [
            "assignee_set",
            "fields_filled:Goal;Steps",
            "ac_present",
            "ac_all_checked",
            "size_at_most:M",
            "blockers_done",
            "not_blocked",
            "link:pr",
            "independent",
            "reason",
            "predicate:code.linked",
        ] {
            assert_eq!(EntryRule::parse(token).expect(token).token(), token);
        }
    }

    #[test]
    fn a_typo_is_refused_not_ignored() {
        for bad in ["lead:x", "roles:wizard", "admins"] {
            assert!(Mover::parse(bad).is_err(), "{bad}");
        }
        for bad in [
            "link:prs",
            "size_at_most:XL",
            "predicate:nope",
            "assigne_set",
        ] {
            assert!(EntryRule::parse(bad).is_err(), "{bad}");
        }
    }
}
