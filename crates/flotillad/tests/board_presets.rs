//! Starting a board from a preset (0.20.0, `board_presets`): `board_enable`
//! with `preset` and `columns`, as the contract's fixtures send them.

use crate::common;

use bus::contract::board::{self as c, board_request::Request};
use bus::contract::wire::{envelope::Body, Envelope};
use common::board::*;
use common::*;
use flotillad::kanban::model::Priority;

/// The `board_enable` a fixture sends, for `project_id`.
fn fixture(json: &str, project_id: &str) -> Request {
    let envelope: Envelope = serde_json::from_str(json).expect("a fixture envelope");
    let Some(Body::BoardRequest(c::BoardRequest {
        request: Some(Request::BoardEnable(mut enable)),
    })) = envelope.body
    else {
        panic!("not a board_enable");
    };
    enable.project_id = project_id.to_string();
    Request::BoardEnable(enable)
}

const PRESET: &str =
    include_str!("../../bus/fixtures/board/messages/request.board_enable.preset.json");
const COLUMNS: &str =
    include_str!("../../bus/fixtures/board/messages/request.board_enable.columns.json");
const RELEASE_GATED: &str =
    include_str!("../../bus/fixtures/board/messages/request.board_enable.release_gated.json");

fn visible(board: &c::BoardSnapshot) -> Vec<(String, String)> {
    board
        .columns
        .iter()
        .filter(|c| c.visible)
        .map(|c| (c.key.clone(), c.name.clone()))
        .collect()
}

fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter()
        .map(|(k, n)| (k.to_string(), n.to_string()))
        .collect()
}

async fn project(name: &str) -> (TestDaemon, WsClient, String) {
    let d = spawn_daemon().await;
    let mut owner = WsClient::connect(&d).await;
    let project_id = common::peers::project(&mut owner, name).await;
    (d, owner, project_id)
}

#[tokio::test]
async fn a_preset_by_name_starts_its_columns() {
    let (_d, mut owner, project_id) = project("Preset").await;
    let board = snapshot(call(&mut owner, fixture(PRESET, &project_id)).await);
    let expected: Vec<String> = flotillad::kanban::presets::Preset::parse(
        serde_json::from_str::<serde_json::Value>(PRESET).unwrap()["boardRequest"]["boardEnable"]
            ["preset"]
            .as_str()
            .unwrap(),
    )
    .unwrap()
    .columns()
    .into_iter()
    .map(|c| c.key)
    .collect();
    let keys: Vec<String> = board.columns.iter().map(|c| c.key.clone()).collect();
    assert_eq!(keys, expected);
}

#[tokio::test]
async fn custom_columns_replace_the_presets() {
    let (d, mut owner, project_id) = project("Custom").await;
    let board = snapshot(call(&mut owner, fixture(COLUMNS, &project_id)).await);
    assert_eq!(
        visible(&board),
        pairs(&[
            ("todo", "Backlog"),
            ("doing", "In progress"),
            ("review", "Review"),
            ("waiting-on-qa", "Waiting on QA"),
            ("done", "Shipped"),
        ])
    );
    let hidden: Vec<&str> = board
        .columns
        .iter()
        .filter(|c| !c.visible)
        .map(|c| c.key.as_str())
        .collect();
    assert_eq!(hidden, ["cancelled"]);
    let doing = board.columns.iter().find(|c| c.key == "doing").unwrap();
    assert_eq!(doing.wip_limit, Some(3));
    let qa = board
        .columns
        .iter()
        .find(|c| c.key == "waiting-on-qa")
        .unwrap();
    assert_eq!(qa.wip_limit, None);
    assert!(qa.rules.is_empty() && qa.movers.is_empty());
    // A new card lands in the first open column.
    let (item_id, _) = new_item(&d.app.db, &project_id, "First", Priority::P2);
    let item = d.app.db.get_item(&item_id).unwrap().expect("the item");
    assert_eq!(item.column_key, "todo");

    // Started once: a second start with other columns changes nothing.
    let again = snapshot(call(&mut owner, fixture(RELEASE_GATED, &project_id)).await);
    assert_eq!(visible(&again), visible(&board));
}

#[tokio::test]
async fn a_release_gated_board_takes_new_names_only() {
    let (_d, mut owner, project_id) = project("Gated").await;
    let board = snapshot(call(&mut owner, fixture(RELEASE_GATED, &project_id)).await);
    assert_eq!(
        visible(&board),
        pairs(&[
            ("inbox", "Ideas"),
            ("ready", "Ready"),
            ("doing", "Building"),
            ("review", "Code review"),
            ("verify", "QA"),
            ("approval", "Awaiting owner"),
            ("deploying", "Shipping"),
            ("done", "Live"),
        ])
    );
    let verify = board.columns.iter().find(|c| c.key == "verify").unwrap();
    assert_eq!(verify.category, c::ColumnCategory::Verify as i32);
}

#[tokio::test]
async fn a_bad_start_is_refused_and_starts_nothing() {
    let (d, mut owner, project_id) = project("Refused").await;
    let unknown = Request::BoardEnable(c::BoardEnable {
        project_id: project_id.clone(),
        preset: Some("scrum".into()),
        columns: Vec::new(),
    });
    assert_eq!(
        error_code(call(&mut owner, unknown).await),
        "invalid_request"
    );
    let no_done = Request::BoardEnable(c::BoardEnable {
        project_id: project_id.clone(),
        preset: Some("simple".into()),
        columns: vec![c::ColumnSpec {
            key: None,
            name: "To do".into(),
            kind: c::ColumnKind::Open as i32,
        }],
    });
    assert_eq!(
        error_code(call(&mut owner, no_done).await),
        "invalid_request"
    );
    assert!(d.app.db.board_settings(&project_id).unwrap().is_none());
}
