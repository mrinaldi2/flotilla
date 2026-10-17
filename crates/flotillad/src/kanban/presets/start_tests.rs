//! `board_enable {preset, columns}` (0.20.0): what a board starts with.

use bus::contract::board as c;

use super::{Preset, Start};
use crate::kanban::model::{ColumnCategory, ColumnKind};

fn spec(key: Option<&str>, name: &str, kind: c::ColumnKind) -> c::ColumnSpec {
    c::ColumnSpec {
        key: key.map(str::to_string),
        name: name.to_string(),
        kind: kind as i32,
    }
}

fn keys(start: &Start) -> Vec<&str> {
    start.columns.iter().map(|c| c.key.as_str()).collect()
}

#[test]
fn no_preset_is_the_first_board() {
    let start = Start::parse(None, &[]).expect("starts");
    assert_eq!(start.preset, Preset::FIRST);
    assert_eq!(
        keys(&start),
        ["todo", "research", "doing", "review", "done", "cancelled"]
    );
}

#[test]
fn every_preset_parses_by_name() {
    for preset in Preset::ALL {
        let start = Start::parse(Some(preset.name()), &[]).expect("starts");
        assert_eq!(start.preset, *preset);
        assert_eq!(start.columns.len(), preset.columns().len());
    }
    let refused = Start::parse(Some("scrum"), &[]).expect_err("unknown preset");
    assert!(refused.contains("scrum"), "{refused}");
}

/// The fixture `request.board_enable.columns.json`: renamed preset columns
/// keep their rules, a new one gets a key from its name and no rules.
#[test]
fn custom_columns_keep_the_presets_rules_by_key() {
    let start = Start::parse(
        Some("simple_review"),
        &[
            spec(Some("todo"), "Backlog", c::ColumnKind::Open),
            spec(Some("doing"), "In progress", c::ColumnKind::Active),
            spec(Some("review"), "Review", c::ColumnKind::Active),
            spec(None, "Waiting on QA", c::ColumnKind::Active),
            spec(Some("done"), "Shipped", c::ColumnKind::Done),
        ],
    )
    .expect("starts");
    assert_eq!(
        keys(&start),
        [
            "todo",
            "doing",
            "review",
            "waiting-on-qa",
            "done",
            "cancelled"
        ]
    );
    let doing = &start.columns[1];
    assert_eq!(doing.name, "In progress");
    assert_eq!(doing.wip_limit, Some(3));
    assert!(!doing.policy.rules.is_empty());
    let qa = &start.columns[3];
    assert_eq!(qa.wip_limit, None);
    assert!(qa.policy.movers.is_empty() && qa.policy.rules.is_empty());
    assert!(!start.columns[5].visible, "Cancelled stays, hidden");
    assert_eq!(start.categories[3], ColumnCategory::Doing);
    assert_eq!(
        start.automations.get("work.started").map(String::as_str),
        Some("doing")
    );
}

#[test]
fn a_dropped_column_takes_its_automations_and_sources_along() {
    let start = Start::parse(
        Some("software"),
        &[
            spec(Some("inbox"), "Inbox", c::ColumnKind::Open),
            spec(Some("review"), "Review", c::ColumnKind::Active),
            spec(Some("done"), "Done", c::ColumnKind::Done),
        ],
    )
    .expect("starts");
    let review = start
        .columns
        .iter()
        .find(|c| c.key == "review")
        .expect("review");
    assert!(review.policy.allowed_from.is_empty(), "doing is gone");
    assert!(start.automations.values().all(|c| c != "doing"));
    assert_eq!(
        start.automations.get("pr.opened").map(String::as_str),
        Some("review")
    );
}

#[test]
fn a_name_never_makes_a_presets_key() {
    let start = Start::parse(
        Some("simple"),
        &[
            spec(None, "Doing", c::ColumnKind::Open),
            spec(None, "Cancelled", c::ColumnKind::Active),
            spec(Some("done"), "Done", c::ColumnKind::Done),
        ],
    )
    .expect("starts");
    assert_eq!(
        keys(&start),
        ["doing-2", "cancelled-2", "done", "cancelled"]
    );
    assert_eq!(start.columns[0].wip_limit, None);
}

#[test]
fn custom_lists_are_checked() {
    let refused =
        |specs: &[c::ColumnSpec]| Start::parse(Some("simple"), specs).expect_err("refused");
    let open = spec(None, "To do", c::ColumnKind::Open);
    let done = spec(None, "Done", c::ColumnKind::Done);
    assert!(refused(std::slice::from_ref(&open)).contains("done column"));
    assert!(refused(std::slice::from_ref(&done)).contains("open column"));
    assert!(refused(&[
        open.clone(),
        spec(None, "done", c::ColumnKind::Done),
        done.clone()
    ])
    .contains("Two columns"));
    assert!(refused(&[
        open.clone(),
        spec(None, "  ", c::ColumnKind::Active),
        done.clone()
    ])
    .contains("name"));
    assert!(refused(&[
        open.clone(),
        spec(None, "Gone", c::ColumnKind::Cancelled),
        done.clone()
    ])
    .contains("Cancelled"));
    assert!(refused(&[
        open.clone(),
        spec(None, "Wait", c::ColumnKind::Unspecified),
        done.clone()
    ])
    .contains("kind"));
    for key in [
        "Doing",
        "to_do",
        "-x",
        "x-",
        "a--b",
        "cancelled",
        &"k".repeat(33),
    ] {
        let listed = spec(Some(key), "Odd", c::ColumnKind::Active);
        assert!(
            Start::parse(Some("simple"), &[open.clone(), listed, done.clone()]).is_err(),
            "{key}"
        );
    }
    let twice = [
        spec(Some("a"), "A", c::ColumnKind::Open),
        spec(Some("a"), "B", c::ColumnKind::Done),
    ];
    assert!(refused(&twice).contains("key"));
}

#[test]
fn long_names_make_short_keys() {
    let start = Start::parse(
        Some("simple"),
        &[
            spec(None, &"Waiting ".repeat(10), c::ColumnKind::Open),
            spec(None, "Done", c::ColumnKind::Done),
        ],
    )
    .expect("starts");
    let key = &start.columns[0].key;
    assert!(key.len() <= 32 && !key.ends_with('-'), "{key}");
    assert_eq!(start.columns[1].key, "done-2", "done is the preset's");
}

/// The fixture `request.board_enable.release_gated.json`: names only.
#[test]
fn release_gated_columns_are_only_renamed() {
    let names = [
        ("inbox", "Ideas"),
        ("ready", "Ready"),
        ("doing", "Building"),
        ("review", "Code review"),
        ("verify", "QA"),
        ("approval", "Awaiting owner"),
        ("deploying", "Shipping"),
        ("done", "Live"),
    ];
    let specs: Vec<c::ColumnSpec> = names
        .iter()
        .map(|(key, name)| spec(Some(key), name, c::ColumnKind::Unspecified))
        .collect();
    let start = Start::parse(Some("release_gated"), &specs).expect("starts");
    let preset = Start::of(Preset::ReleaseGated);
    assert_eq!(keys(&start), keys(&preset));
    assert_eq!(start.categories, preset.categories);
    // Listed without Research (a client before 0.21.0), Research keeps its
    // name and Doing is the fourth column.
    assert_eq!(start.columns[2].name, "Research");
    assert_eq!(start.columns[3].name, "Building");
    assert_eq!(start.columns[3].wip_limit, preset.columns[3].wip_limit);

    let mut moved = specs.clone();
    moved.swap(0, 1);
    assert!(Start::parse(Some("release_gated"), &moved).is_err());
    assert!(Start::parse(Some("release_gated"), &specs[..7]).is_err());
    let mut rekinded = specs.clone();
    rekinded[0].kind = c::ColumnKind::Done as i32;
    assert!(Start::parse(Some("release_gated"), &rekinded).is_err());
    let mut same_kind = specs;
    same_kind[0].kind = c::ColumnKind::Open as i32;
    same_kind[0].key = None;
    let start = Start::parse(Some("release_gated"), &same_kind).expect("same kind, no key");
    assert_eq!(start.columns[0].kind, ColumnKind::Open);
    assert_eq!(start.columns[0].name, "Ideas");
}
