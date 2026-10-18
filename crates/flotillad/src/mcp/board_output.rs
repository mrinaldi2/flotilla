//! Contract output as a bot reads it: the proto's field names, short enum
//! names, and none of the fields kept only for 0.18 readers.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde_json::{json, Value};

use crate::kanban::model::Item;

use super::board::item_out;

use crate::kanban::contract::all_spellings;

/// proto3 JSON's lowerCamelCase field names back to the proto's own, which
/// are also the argument names.
fn snake(key: &str) -> String {
    let mut out = String::with_capacity(key.len() + 4);
    for ch in key.chars() {
        if ch.is_ascii_uppercase() {
            out.push('_');
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

/// Contract output for a bot: field names as in the proto (`column_key`),
/// and wire enum names become the short ones.
pub(super) fn friendly(mut value: Value) -> Value {
    static SHORT: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    let short = SHORT.get_or_init(|| {
        all_spellings()
            .into_iter()
            .chain(crate::plan::contract::spellings())
            .chain(crate::questions::wire::all_spellings())
            .map(|(short, wire)| (wire, short))
            .collect()
    });
    fn walk(v: &mut Value, short: &HashMap<&'static str, &'static str>) {
        match v {
            Value::String(s) => {
                if let Some(name) = short.get(s.as_str()) {
                    *s = (*name).to_string();
                }
            }
            Value::Array(items) => items.iter_mut().for_each(|i| walk(i, short)),
            Value::Object(map) => {
                // Board settings' machines keyed by bare platform, beside
                // the same by label.
                let by_label = map.contains_key("requiredMachinesByLabel")
                    || map.contains_key("required_machines_by_label");
                *map = std::mem::take(map)
                    .into_iter()
                    .map(|(key, mut value)| {
                        walk(&mut value, short);
                        (snake(&key), value)
                    })
                    // Filled for 0.18 readers; a bot reads the labels.
                    .filter(|(key, _)| {
                        key != "platforms" && !(by_label && key == "required_machines")
                    })
                    .collect();
            }
            _ => {}
        }
    }
    walk(&mut value, short);
    value
}

pub(super) fn conflict(current: Item) -> anyhow::Error {
    let version = current.version;
    let item = item_out(current).unwrap_or(Value::Null);
    Conflict { version, item }.into()
}

/// A stale write and the item as it is now. Typed, so the board's home can
/// send it to a linked computer as data and have its ids rewritten like any
/// result's, rather than as text that keeps the home's ids (H-113).
#[derive(Debug)]
pub struct Conflict {
    pub version: u64,
    pub item: Value,
}

impl Conflict {
    /// As the home sends it in a `board_call` result.
    pub fn to_result(&self) -> Value {
        json!({ "conflict": self.item, "version": self.version })
    }

    /// From such a result, once its ids are this computer's.
    pub fn from_result(result: &Value) -> Option<Self> {
        let item = result.get("conflict").filter(|c| c.is_object())?;
        Some(Self {
            version: result["version"].as_u64()?,
            item: item.clone(),
        })
    }
}

impl std::fmt::Display for Conflict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let version = self.version;
        write!(
            f,
            "conflict: the item changed since you read it; it is now version {version}. \
             Check it and retry with expected_version {version}.\n{}",
            self.item
        )
    }
}

impl std::error::Error for Conflict {}
