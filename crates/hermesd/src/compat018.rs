//! What 0.18 apps and linked computers still send and read (0.19.0 K8).
//!
//! Before 0.19 an item had platforms from a fixed set (desktop, ios, daemon,
//! infra). They are now its labels in the `platform` group (`platform:ios`),
//! and the board knows nothing else about them. The wire keeps the old
//! fields for one minor release: a 0.19 home fills them from the labels for
//! 0.18 readers, and maps them to labels when a 0.18 client or peer sends
//! them. Nothing else in the daemon reads them.

use std::collections::BTreeMap;

use bus::contract::board as c;

use crate::kanban::labels::{self, label};

/// The label group the old platforms became.
pub const GROUP: &str = "platform";

/// The old platforms, by wire number, with their stored names.
const NAMES: [(c::Platform, &str); 4] = [
    (c::Platform::Desktop, "desktop"),
    (c::Platform::Ios, "ios"),
    (c::Platform::Daemon, "daemon"),
    (c::Platform::Infra, "infra"),
];

fn name_of(number: i32) -> Option<&'static str> {
    NAMES
        .iter()
        .find(|(p, _)| *p as i32 == number)
        .map(|(_, name)| *name)
}

fn number_of(name: &str) -> Option<i32> {
    NAMES
        .iter()
        .find(|(_, n)| *n == name)
        .map(|(p, _)| *p as i32)
}

/// The values of the `platform` group the old numbers stand for; numbers
/// a 0.19 home doesn't know are dropped.
fn values_of(platforms: &[i32]) -> Vec<String> {
    platforms
        .iter()
        .filter_map(|n| name_of(*n))
        .map(str::to_string)
        .collect()
}

/// The labels plus those the old platforms stand for, each once.
fn with_platforms(labels: Vec<String>, platforms: &[i32]) -> Vec<String> {
    let extra = values_of(platforms).into_iter().map(|v| label(GROUP, &v));
    labels::dedup(labels.into_iter().chain(extra))
}

/// The old platforms the labels carry: those of the `platform` group a
/// 0.18 reader knows, in label order.
fn platforms_of(labels: &[String]) -> Vec<i32> {
    labels::values_in(labels, GROUP)
        .into_iter()
        .filter_map(number_of)
        .collect()
}

/// An item as sent: its platforms filled for 0.18 readers.
pub fn item_out(item: &mut c::Item) {
    item.platforms = platforms_of(&item.labels);
}

pub fn card_out(card: &mut c::ItemCard) {
    card.platforms = platforms_of(&card.labels);
}

/// An item as received, from a home on either version: platforms become
/// labels.
pub fn item_in(item: &mut c::Item) {
    let platforms = std::mem::take(&mut item.platforms);
    item.labels = with_platforms(std::mem::take(&mut item.labels), &platforms);
}

pub fn card_in(card: &mut c::ItemCard) {
    let platforms = std::mem::take(&mut card.platforms);
    card.labels = with_platforms(std::mem::take(&mut card.labels), &platforms);
}

/// A new item's labels, with any platforms a 0.18 caller named.
pub fn create_labels(req: &c::ItemCreate) -> Vec<String> {
    with_platforms(req.labels.clone(), &req.platforms)
}

/// The label group an update replaces, when a 0.18 caller sets platforms.
pub fn update_group(req: &c::ItemUpdate) -> Option<(&'static str, Vec<String>)> {
    req.platforms
        .as_ref()
        .map(|list| (GROUP, values_of(&list.values)))
}

/// The labels a query asks for, with any platforms a 0.18 caller named.
pub fn query_labels(req: &c::ItemQuery) -> Vec<String> {
    with_platforms(req.labels.clone(), &req.platforms)
}

/// The machines by label, as a 0.18 reader keys them: only the labels of
/// the old platforms, by bare name (any other key would fail its read).
pub fn machines_out(by_label: &BTreeMap<String, Vec<String>>) -> BTreeMap<String, c::StringList> {
    by_label
        .iter()
        .filter_map(|(key, machines)| {
            let (group, value) = labels::split(key)?;
            (group == GROUP && number_of(value).is_some()).then(|| {
                (
                    value.to_string(),
                    c::StringList {
                        values: machines.clone(),
                    },
                )
            })
        })
        .collect()
}

/// The machines by label from settings as received: a 0.19 home's own
/// map, or a 0.18 home's platform keys as `platform:` labels.
pub fn machines_in(settings: &c::BoardSettings) -> BTreeMap<String, Vec<String>> {
    let (map, keyed_by_platform) = if settings.required_machines_by_label.is_empty() {
        (&settings.required_machines, true)
    } else {
        (&settings.required_machines_by_label, false)
    };
    map.iter()
        .map(|(key, list)| {
            let key = if keyed_by_platform && labels::split(key).is_none() {
                label(GROUP, key)
            } else {
                key.clone()
            };
            (key, list.values.clone())
        })
        .collect()
}

/// The old platforms' spellings, short and wire, for decoding a 0.18
/// caller's tool arguments.
pub fn spellings(enum_name: &str) -> Option<Vec<(&'static str, &'static str)>> {
    (enum_name == "Platform").then(|| {
        NAMES
            .iter()
            .map(|(p, name)| (*name, p.as_str_name()))
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owned(labels: &[&str]) -> Vec<String> {
        labels.iter().map(|l| (*l).to_string()).collect()
    }

    #[test]
    fn a_018_item_reads_as_labels_and_is_sent_back_with_its_platforms() {
        let mut item = c::Item {
            labels: owned(&["urgent", "platform:ios"]),
            platforms: vec![c::Platform::Ios as i32, c::Platform::Daemon as i32, 99],
            ..Default::default()
        };
        item_in(&mut item);
        assert_eq!(item.labels, ["urgent", "platform:ios", "platform:daemon"]);
        assert!(item.platforms.is_empty());
        item.labels.push("platform:android".into());
        item_out(&mut item);
        assert_eq!(
            item.platforms,
            [c::Platform::Ios as i32, c::Platform::Daemon as i32]
        );
    }

    #[test]
    fn requests_with_platforms_become_label_changes() {
        let create = c::ItemCreate {
            labels: owned(&["urgent"]),
            platforms: vec![c::Platform::Infra as i32],
            ..Default::default()
        };
        assert_eq!(create_labels(&create), ["urgent", "platform:infra"]);
        let update = c::ItemUpdate {
            platforms: Some(c::PlatformList {
                values: vec![c::Platform::Desktop as i32],
            }),
            ..Default::default()
        };
        assert_eq!(update_group(&update), Some((GROUP, owned(&["desktop"]))));
        assert_eq!(update_group(&c::ItemUpdate::default()), None);
        let query = c::ItemQuery {
            platforms: vec![c::Platform::Ios as i32],
            ..Default::default()
        };
        assert_eq!(query_labels(&query), ["platform:ios"]);
    }

    #[test]
    fn machines_keep_their_labels_and_018_readers_get_the_platforms() {
        let by_label: BTreeMap<String, Vec<String>> = [
            ("platform:ios".to_string(), owned(&["iphone"])),
            ("platform:android".to_string(), owned(&["pixel"])),
            ("area:web".to_string(), owned(&["mac"])),
        ]
        .into();
        let old = machines_out(&by_label);
        assert_eq!(old.keys().collect::<Vec<_>>(), ["ios"]);
        let from_018 = c::BoardSettings {
            required_machines: old,
            ..Default::default()
        };
        assert_eq!(
            machines_in(&from_018),
            BTreeMap::from([("platform:ios".to_string(), owned(&["iphone"]))])
        );
        let from_019 = c::BoardSettings {
            required_machines_by_label: by_label
                .iter()
                .map(|(k, v)| (k.clone(), c::StringList { values: v.clone() }))
                .collect(),
            ..Default::default()
        };
        assert_eq!(machines_in(&from_019), by_label);
    }
}
