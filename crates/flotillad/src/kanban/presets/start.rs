//! What a new board starts with (0.20.0, `board_presets`): a preset's
//! columns, or the owner's list in place of its visible ones, checked
//! before anything is written. The rules are `BoardEnable`'s
//! (proto/flotilla/board/v1/requests.proto).

use std::collections::{BTreeMap, BTreeSet};

use bus::contract::board as c;

use super::{Preset, PresetColumn};
use crate::kanban::flow::ColumnPolicy;
use crate::kanban::model::{BoardFlow, ColumnCategory, ColumnKind, WipScope};

/// The longest column key a list may name, or a name may make.
const KEY_MAX: usize = 32;
/// The hidden column every preset keeps last. It is never listed.
const CANCELLED: &str = "cancelled";

/// A new board's columns, their categories and its automations.
#[derive(Debug, Clone)]
pub struct Start {
    pub preset: Preset,
    pub columns: Vec<PresetColumn>,
    /// Each column's category, in `columns` order.
    pub categories: Vec<ColumnCategory>,
    /// Fact → column key.
    pub automations: BTreeMap<String, String>,
}

impl Start {
    /// The preset as it is.
    pub fn of(preset: Preset) -> Self {
        Self {
            preset,
            columns: preset.columns(),
            categories: preset.categories(),
            automations: preset.automations(),
        }
    }

    /// `board_enable {preset?, columns?}`: the preset by name (left out: a
    /// project's first board), with `specs` in place of its visible columns
    /// when there are any. The error is the sentence to refuse with.
    pub fn parse(preset: Option<&str>, specs: &[c::ColumnSpec]) -> Result<Self, String> {
        let preset = match preset {
            None => Preset::FIRST,
            Some(name) => Preset::parse(name).ok_or_else(|| {
                format!(
                    "There's no board preset '{name}': use simple, simple_review, \
                     software or release_gated."
                )
            })?,
        };
        if specs.is_empty() {
            return Ok(Self::of(preset));
        }
        match preset.flow() {
            BoardFlow::ReleaseGated => renamed(preset, specs),
            BoardFlow::Custom => custom(preset, specs),
        }
    }
}

/// A release-gated board's guards key on its columns, so only their names
/// change: the list is its visible columns, in order.
fn renamed(preset: Preset, specs: &[c::ColumnSpec]) -> Result<Start, String> {
    let mut start = Start::of(preset);
    let mut visible: Vec<usize> = (0..start.columns.len())
        .filter(|&i| start.columns[i].visible)
        .collect();
    // A client from before 0.21 lists the columns without Research, which
    // then keeps its name.
    let named_research = specs.iter().any(|s| s.key.as_deref() == Some("research"));
    if specs.len() + 1 == visible.len() && !named_research {
        visible.retain(|&i| !start.columns[i].research);
    }
    if specs.len() != visible.len() {
        return Err(format!(
            "A release-gated board keeps its {} columns: list each of them, in order, \
             to rename them.",
            visible.len()
        ));
    }
    for (spec, &i) in specs.iter().zip(&visible) {
        let name = name(spec)?;
        let column = &mut start.columns[i];
        if spec.key.as_deref().is_some_and(|key| key != column.key) {
            return Err(format!(
                "A release-gated board's columns can only be renamed: '{name}' is \
                 its '{}' column.",
                column.key
            ));
        }
        if kind(spec)?.is_some_and(|kind| kind != column.kind) {
            return Err(format!(
                "A release-gated board's columns keep their kinds: '{name}' is {}.",
                column.kind.as_str()
            ));
        }
        column.name = name;
    }
    Ok(start)
}

/// A custom board's visible columns, as listed. A column with one of the
/// preset's keys takes that column's limit and rules; any other has none.
fn custom(preset: Preset, specs: &[c::ColumnSpec]) -> Result<Start, String> {
    let template = preset.columns();
    let cancelled = template
        .iter()
        .find(|column| column.key == CANCELLED)
        .cloned()
        .ok_or_else(|| format!("The {} preset has no Cancelled column.", preset.name()))?;
    let mut names = BTreeSet::new();
    let mut listed = BTreeSet::new();
    let mut kinds = Vec::new();
    for spec in specs {
        let name = name(spec)?;
        if !names.insert(name.to_lowercase()) {
            return Err(format!("Two columns are named '{name}'."));
        }
        let kind = match kind(spec)? {
            Some(kind @ (ColumnKind::Open | ColumnKind::Active | ColumnKind::Done)) => kind,
            Some(ColumnKind::Cancelled) => {
                return Err(
                    "Every board keeps a hidden Cancelled column: list only the visible ones."
                        .to_string(),
                )
            }
            None => return Err(format!("Pick a kind for '{name}': open, active or done.")),
        };
        if let Some(key) = &spec.key {
            check_key(key)?;
            if !listed.insert(key.clone()) {
                return Err(format!("Two columns have the key '{key}'."));
            }
        }
        kinds.push(kind);
    }
    if !kinds.contains(&ColumnKind::Open) || !kinds.contains(&ColumnKind::Done) {
        return Err("A board needs at least one open column and one done column.".to_string());
    }
    // A key made from a name is never one of the preset's: only a listed key
    // takes a preset column's rules.
    let mut taken: BTreeSet<String> = listed;
    taken.extend(template.iter().map(|column| column.key.clone()));
    let mut columns = Vec::with_capacity(specs.len() + 1);
    for (spec, kind) in specs.iter().zip(kinds) {
        let name = name(spec)?;
        let key = match &spec.key {
            Some(key) => key.clone(),
            None => {
                let key = free_key(&slug(&name), &taken);
                taken.insert(key.clone());
                key
            }
        };
        let column = match template.iter().find(|t| t.key == key && t.visible) {
            Some(from) => PresetColumn {
                key,
                name,
                kind,
                ..from.clone()
            },
            None => PresetColumn {
                key,
                name,
                kind,
                wip_limit: None,
                wip_scope: WipScope::Column,
                visible: true,
                policy: ColumnPolicy::default(),
                research: false,
            },
        };
        columns.push(column);
    }
    columns.push(cancelled);
    let keys: BTreeSet<String> = columns.iter().map(|column| column.key.clone()).collect();
    for column in &mut columns {
        column
            .policy
            .allowed_from
            .retain(|from| keys.contains(from));
    }
    // A list without Research (from a client before 0.21.0) starts work in
    // Doing, as it did.
    let automations = preset
        .automations()
        .into_iter()
        .map(|(fact, column)| match column.as_str() {
            "research" if !keys.contains(&column) => (fact, "doing".to_string()),
            _ => (fact, column),
        })
        .filter(|(_, column)| keys.contains(column))
        .collect();
    Ok(Start {
        preset,
        categories: columns.iter().map(PresetColumn::category).collect(),
        columns,
        automations,
    })
}

fn name(spec: &c::ColumnSpec) -> Result<String, String> {
    let name = spec.name.trim();
    if name.is_empty() {
        return Err("Every column needs a name.".to_string());
    }
    Ok(name.to_string())
}

/// The listed kind; `None` when left unspecified.
fn kind(spec: &c::ColumnSpec) -> Result<Option<ColumnKind>, String> {
    if spec.kind == c::ColumnKind::Unspecified as i32 {
        return Ok(None);
    }
    ColumnKind::from_wire(spec.kind)
        .map(Some)
        .map_err(|_| format!("'{}' has an unknown column kind.", spec.name.trim()))
}

/// Lowercase letters and digits in words joined by `-`, at most 32
/// characters, and not the hidden Cancelled column's.
fn check_key(key: &str) -> Result<(), String> {
    let words = key.split('-').all(|word| {
        !word.is_empty()
            && word
                .chars()
                .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit())
    });
    if !words || key.len() > KEY_MAX {
        return Err(format!(
            "'{key}' isn't a column key: use lowercase letters and digits joined by '-', \
             at most {KEY_MAX} characters."
        ));
    }
    if key == CANCELLED {
        return Err("The key 'cancelled' is the hidden Cancelled column's.".to_string());
    }
    Ok(())
}

/// A key from a name, as `column_add` makes one: "Waiting on QA" is
/// `waiting-on-qa`.
fn slug(name: &str) -> String {
    let words: Vec<String> = name
        .to_lowercase()
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|word| !word.is_empty())
        .map(str::to_string)
        .collect();
    let slug = cut(&words.join("-"), KEY_MAX);
    if slug.is_empty() {
        "column".to_string()
    } else {
        slug
    }
}

/// `base`, or `base-2`, `base-3`… when taken, within 32 characters.
fn free_key(base: &str, taken: &BTreeSet<String>) -> String {
    if !taken.contains(base) && base != CANCELLED {
        return base.to_string();
    }
    (2..)
        .map(|n| {
            let suffix = format!("-{n}");
            format!("{}{suffix}", cut(base, KEY_MAX - suffix.len()))
        })
        .find(|key| !taken.contains(key))
        .unwrap_or_else(|| base.to_string())
}

/// An ASCII key cut to `max` characters, without a dangling `-`.
fn cut(key: &str, max: usize) -> String {
    key.chars()
        .take(max)
        .collect::<String>()
        .trim_end_matches('-')
        .to_string()
}
