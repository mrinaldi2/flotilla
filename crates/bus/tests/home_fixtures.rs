//! Golden proto-JSON fixtures for the projects home (H-128 rev 2): each
//! decodes with the generated types, survives a proto-JSON and a binary round
//! trip, and keeps the proto field names (snake_case) on the wire. The
//! desktop and iOS clients decode the same files.

use std::path::PathBuf;

use bus::contract::board::ColumnKind;
use bus::contract::home::{
    attention_row::Target, AttentionKind, AttentionRows, BotBrief, BotStatus, ProjectAttention,
    ProjectRow, ProjectsOverview,
};
use prost::Message;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

fn read(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("fixtures/home/{name}.json"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn round_trip<T>(name: &str) -> Value
where
    T: DeserializeOwned + Serialize + Message + Default + PartialEq + std::fmt::Debug,
{
    let decoded: T = serde_json::from_str(&read(name))
        .unwrap_or_else(|e| panic!("{name}.json does not decode: {e}"));
    let json = serde_json::to_value(&decoded).expect("encode");
    let via_json: T = serde_json::from_value(json.clone()).expect("re-decode");
    assert_eq!(
        via_json, decoded,
        "{name}.json changes on a JSON round trip"
    );
    let via_binary = T::decode(decoded.encode_to_vec().as_slice()).expect("binary decode");
    assert_eq!(
        via_binary, decoded,
        "{name}.json changes on a binary round trip"
    );
    json
}

#[test]
fn every_home_fixture_round_trips_with_proto_field_names() {
    let overview = round_trip::<ProjectsOverview>("overview");
    assert!(
        overview["rows"][0].get("project_id").is_some(),
        "{overview}"
    );
    assert!(overview["rows"][0].get("projectId").is_none());
    assert_eq!(overview["sources"][0]["state"], "OFFLINE");
    let rows = round_trip::<AttentionRows>("attention_rows");
    assert_eq!(rows["rows"][0]["kind"], "OWNER_ACTION");
    round_trip::<ProjectAttention>("project_attention");
}

/// `by_kind` keys are the kinds' names in lowercase.
#[test]
fn by_kind_keys_are_lowercase_kind_names() {
    let overview: ProjectsOverview = serde_json::from_str(&read("overview")).expect("decodes");
    let total = overview.total.expect("total");
    for key in total.by_kind.keys() {
        let kind = AttentionKind::from_str_name(&key.to_ascii_uppercase());
        assert!(kind.is_some(), "{key} is no AttentionKind");
    }
}

fn overview() -> ProjectsOverview {
    serde_json::from_str(&read("overview")).expect("decodes")
}

fn project<'a>(overview: &'a ProjectsOverview, id: &str) -> &'a ProjectRow {
    overview
        .rows
        .iter()
        .find(|r| r.project_id == id)
        .unwrap_or_else(|| panic!("no row {id}"))
}

/// `bot_list`'s group order (home.proto): the lead, then waiting, working,
/// failed and stopped, idle, offline.
fn group(bot: &BotBrief) -> u8 {
    if bot.lead {
        return 0;
    }
    match bot.status() {
        BotStatus::Waiting => 1,
        BotStatus::Working => 2,
        BotStatus::Failed | BotStatus::Stopped => 3,
        BotStatus::Idle => 4,
        BotStatus::Offline => 5,
        BotStatus::Unspecified => panic!("{} has no status", bot.name),
    }
}

/// The Board glance's bots (0.20.0): every status, in the documented order,
/// enums by their prefixed names, a linked computer's bots with its name.
#[test]
fn the_bot_list_covers_every_status_in_order() {
    let json = round_trip::<ProjectsOverview>("overview");
    assert_eq!(
        json["rows"][0]["bot_list"][0]["status"],
        "BOT_STATUS_WORKING"
    );
    let overview = overview();
    let gravity = project(&overview, "p-gravity");
    let bots = &gravity.bot_list;
    assert!(bots[0].lead, "the lead comes first");
    assert_eq!(bots.iter().filter(|b| b.lead).count(), 1);
    let groups: Vec<u8> = bots.iter().map(group).collect();
    assert!(groups.windows(2).all(|w| w[0] <= w[1]), "{groups:?}");
    let offline = bots.last().expect("bots");
    assert_eq!(offline.status(), BotStatus::Offline);
    assert_eq!(offline.daemon_id, "d-imac");
    assert_eq!(offline.computer_name, "imac");
    assert!(bots.len() <= 24 && gravity.bots as usize >= bots.len());

    let mut seen: Vec<BotStatus> = overview
        .rows
        .iter()
        .flat_map(|r| &r.bot_list)
        .map(BotBrief::status)
        .collect();
    seen.sort_by_key(|s| *s as i32);
    seen.dedup();
    let every: Vec<BotStatus> = (1..).map_while(|n| BotStatus::try_from(n).ok()).collect();
    assert_eq!(seen, every, "every status is in the fixture");
    for bot in overview.rows.iter().flat_map(|r| &r.bot_list) {
        assert!(
            ["claude_code", "codex_cli"].contains(&bot.runtime.as_str()),
            "{}",
            bot.runtime
        );
        assert!(bot.since.is_some(), "{} has no since", bot.name);
        assert!(bus::avatar::parse(&bot.avatar).is_ok(), "{}", bot.avatar);
    }
}

/// The first Needs-you row is a stopped bot, and it is in the bot list.
#[test]
fn top_attention_is_a_crashed_bot() {
    let overview = overview();
    let gravity = project(&overview, "p-gravity");
    let top = gravity.top_attention.as_ref().expect("top_attention");
    assert_eq!(top.kind(), AttentionKind::BotStopped);
    assert_eq!(top.bot_state, "crashed");
    let Some(Target::Bot(bot)) = &top.target else {
        panic!("bot_stopped targets a bot: {top:?}");
    };
    let listed = gravity
        .bot_list
        .iter()
        .find(|b| b.bot_id == bot.bot_id && b.daemon_id == bot.daemon_id)
        .expect("the stopped bot is listed");
    assert_eq!(listed.status(), BotStatus::Failed);
    assert_eq!(listed.state, top.bot_state);
    let summary = gravity.attention.as_ref().expect("attention");
    assert_eq!(summary.by_kind.get("bot_stopped"), Some(&1));
    assert!(
        overview.rows[1..].iter().all(|r| r.top_attention.is_none()),
        "nothing needs the owner elsewhere"
    );
}

/// Members' presence, the columns' kinds and the hidden Cancelled column,
/// and a project still being set up.
#[test]
fn members_columns_and_a_project_being_set_up() {
    let overview = overview();
    let gravity = project(&overview, "p-gravity");
    let (mac, imac) = (&gravity.members[0], &gravity.members[1]);
    assert!(mac.online && mac.last_seen_at.is_none());
    assert!(!imac.online && imac.last_seen_at.is_some());

    let kinds =
        |row: &ProjectRow| -> Vec<String> { row.columns.iter().map(|c| c.kind.clone()).collect() };
    let release_gated = kinds(gravity);
    assert_eq!(
        release_gated,
        [
            "open",
            "open",
            "active",
            "active",
            "active",
            "active",
            "active",
            "done",
            "cancelled"
        ]
    );
    for row in &overview.rows {
        for column in &row.columns {
            let name = format!("COLUMN_KIND_{}", column.kind.to_ascii_uppercase());
            assert!(
                ColumnKind::from_str_name(&name).is_some(),
                "{}",
                column.kind
            );
            assert_eq!(column.hidden, column.kind == "cancelled", "{}", column.key);
        }
    }

    let setting_up = project(&overview, "p-calm");
    assert!(setting_up.board_home.is_empty() && setting_up.columns.is_empty());
    assert!(setting_up.bot_list.is_empty() && setting_up.top_attention.is_none());
    assert_eq!(setting_up.bots, 0);
}

/// A linked computer's part lists its own bots; Needs-you rows for stopped
/// bots carry the bot and why.
#[test]
fn parts_list_their_bots_and_stopped_bots_say_why() {
    let attention: ProjectAttention =
        serde_json::from_str(&read("project_attention")).expect("decodes");
    let part = &attention.parts[0];
    assert!(!part.bots.is_empty());
    assert!(part.bots.iter().all(|b| b.daemon_id == "d-imac" && !b.lead));

    let rows: AttentionRows = serde_json::from_str(&read("attention_rows")).expect("decodes");
    let stopped: Vec<_> = rows
        .rows
        .iter()
        .filter(|r| r.kind() == AttentionKind::BotStopped)
        .collect();
    assert_eq!(stopped.len(), 2);
    for row in stopped {
        assert!(matches!(row.target, Some(Target::Bot(_))), "{row:?}");
        assert!(
            ["crashed", "auth_failed", "rate_limited", "stopped"].contains(&row.bot_state.as_str()),
            "{}",
            row.bot_state
        );
    }
    let weights: Vec<u32> = rows.rows.iter().map(|r| r.weight).collect();
    assert!(weights.windows(2).all(|w| w[0] >= w[1]), "{weights:?}");
}
