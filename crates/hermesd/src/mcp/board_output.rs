//! Contract output as a bot reads it: the proto's field names, short enum
//! names, and none of the fields kept only for 0.18 readers.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde_json::Value;

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
                *map = std::mem::take(map)
                    .into_iter()
                    .map(|(key, mut value)| {
                        walk(&mut value, short);
                        (snake(&key), value)
                    })
                    // Filled for 0.18 readers; a bot reads the labels.
                    .filter(|(key, _)| key != "platforms")
                    .collect();
            }
            _ => {}
        }
    }
    walk(&mut value, short);
    value
}
