//! Labels and label groups (0.19.0 K8). An item carries free labels; a
//! label `<group>:<value>` belongs to the group `<group>`, which a board may
//! declare with the values it offers (`area: web, api`), so the apps can
//! filter on it and offer its values.

use serde::{Deserialize, Serialize};

/// A board's label group: its name and the values it offers, in order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabelGroup {
    pub name: String,
    pub values: Vec<String>,
}

impl LabelGroup {
    /// The label one of its values stands for: `area` and `web` give
    /// `area:web`.
    pub fn label(&self, value: &str) -> String {
        label(&self.name, value)
    }
}

/// The label `value` of `group` stands for.
pub fn label(group: &str, value: &str) -> String {
    format!("{group}:{value}")
}

/// The group a label belongs to and its value there, or `None` for a plain
/// label.
pub fn split(label: &str) -> Option<(&str, &str)> {
    label
        .split_once(':')
        .filter(|(group, value)| !group.is_empty() && !value.is_empty())
}

/// The values the labels give the group, in label order.
pub fn values_in<'a>(labels: &'a [String], group: &str) -> Vec<&'a str> {
    labels
        .iter()
        .filter_map(|l| split(l))
        .filter(|(g, _)| *g == group)
        .map(|(_, v)| v)
        .collect()
}

/// Whether any of the labels belongs to the group.
pub fn has_group(labels: &[String], group: &str) -> bool {
    !values_in(labels, group).is_empty()
}

/// The labels with the group's replaced by `values`, which go where the
/// group's first label was (else at the end). Duplicates are dropped.
pub fn replace_group(labels: &[String], group: &str, values: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(labels.len() + values.len());
    let mut placed = false;
    for l in labels {
        if split(l).is_some_and(|(g, _)| g == group) {
            if !placed {
                out.extend(values.iter().map(|v| label(group, v)));
                placed = true;
            }
        } else {
            out.push(l.clone());
        }
    }
    if !placed {
        out.extend(values.iter().map(|v| label(group, v)));
    }
    dedup(out)
}

/// The labels in order, each once, blank ones dropped and the rest trimmed.
pub fn dedup(labels: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for l in labels {
        let l = l.trim();
        if !l.is_empty() && !out.iter().any(|o| o == l) {
            out.push(l.to_string());
        }
    }
    out
}

/// Whether the card carries any of the wanted labels; none wanted matches.
pub fn matches_any(labels: &[String], wanted: &[String]) -> bool {
    wanted.is_empty() || wanted.iter().any(|w| labels.contains(w))
}

/// A name a group may have: lowercase letters, digits, `-` and `_`.
fn is_name(text: &str) -> bool {
    !text.is_empty()
        && text
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '-' | '_'))
}

/// Label groups as a board stores them: names well formed and unique,
/// values trimmed, unique and without colons.
pub fn checked(groups: Vec<LabelGroup>) -> Result<Vec<LabelGroup>, String> {
    let mut out: Vec<LabelGroup> = Vec::with_capacity(groups.len());
    for g in groups {
        let name = g.name.trim().to_string();
        if !is_name(&name) {
            return Err(format!(
                "“{name}” can't name a label group: use lowercase letters, digits, - and _"
            ));
        }
        if out.iter().any(|o| o.name == name) {
            return Err(format!("two label groups are named “{name}”"));
        }
        let values = dedup(g.values);
        if let Some(bad) = values.iter().find(|v| v.contains(':')) {
            return Err(format!("“{bad}” can't be a value of {name}: no colons"));
        }
        out.push(LabelGroup { name, values });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owned(labels: &[&str]) -> Vec<String> {
        labels.iter().map(|l| (*l).to_string()).collect()
    }

    #[test]
    fn a_label_names_its_group_by_prefix() {
        assert_eq!(split("area:web"), Some(("area", "web")));
        assert_eq!(split("wip-override"), None);
        assert_eq!(split(":web"), None);
        let labels = owned(&["area:web", "urgent", "area:api", "team:core"]);
        assert_eq!(values_in(&labels, "area"), ["web", "api"]);
        assert!(has_group(&labels, "team"));
        assert!(!has_group(&labels, "os"));
    }

    #[test]
    fn replacing_a_group_keeps_the_other_labels_in_place() {
        let labels = owned(&["urgent", "area:web", "team:core", "area:api"]);
        assert_eq!(
            replace_group(&labels, "area", &owned(&["db"])),
            ["urgent", "area:db", "team:core"]
        );
        assert_eq!(replace_group(&labels, "area", &[]), ["urgent", "team:core"]);
        assert_eq!(
            replace_group(&owned(&["urgent"]), "os", &owned(&["mac", "mac"])),
            ["urgent", "os:mac"]
        );
    }

    #[test]
    fn groups_are_checked_before_they_are_stored() {
        let group = |name: &str, values: &[&str]| LabelGroup {
            name: name.into(),
            values: owned(values),
        };
        assert_eq!(
            checked(vec![group(" area ", &["web", " web", "", "api"])]).unwrap(),
            [group("area", &["web", "api"])]
        );
        assert!(checked(vec![group("Area", &[])]).is_err());
        assert!(checked(vec![group("a b", &[])]).is_err());
        assert!(checked(vec![group("area", &[]), group("area", &[])]).is_err());
        assert!(checked(vec![group("area", &["a:b"])]).is_err());
    }

    #[test]
    fn a_query_matches_any_wanted_label() {
        let labels = owned(&["area:web"]);
        assert!(matches_any(&labels, &[]));
        assert!(matches_any(&labels, &owned(&["area:api", "area:web"])));
        assert!(!matches_any(&labels, &owned(&["area:api"])));
    }
}
