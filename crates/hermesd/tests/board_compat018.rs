//! A 0.18 app on a 0.19 home (0.19.0 K8). It still sends and reads
//! platforms; the home keeps them as `platform:` labels. Sent together,
//! `labels` replaces the whole list and `platforms` only the platform
//! labels, so a 0.18 client neither loses labels nor doubles them.

use crate::common;

use bus::contract::board::{self as c, board_request::Request, board_response::Response};
use bus::contract::wire::envelope::Body;
use common::board::{call, response};
use common::*;

const IOS: i32 = c::Platform::Ios as i32;
const DESKTOP: i32 = c::Platform::Desktop as i32;
const DAEMON: i32 = c::Platform::Daemon as i32;

fn owned(labels: &[&str]) -> Vec<String> {
    labels.iter().map(|l| (*l).to_string()).collect()
}

fn edited(body: Body) -> c::Item {
    match response(body) {
        Response::Edited(c::EditResult {
            outcome: Some(c::edit_result::Outcome::Done(item)),
            ..
        }) => item,
        other => panic!("expected the item, got {other:?}"),
    }
}

async fn update(owner: &mut WsClient, item: &c::Item, req: c::ItemUpdate) -> c::Item {
    edited(
        call(
            owner,
            Request::ItemUpdate(c::ItemUpdate {
                id: item.id.clone(),
                expected_version: item.version,
                ..req
            }),
        )
        .await,
    )
}

fn platforms(values: &[i32]) -> Option<c::PlatformList> {
    Some(c::PlatformList {
        values: values.to_vec(),
    })
}

fn labels(values: &[&str]) -> Option<c::StringList> {
    Some(c::StringList {
        values: owned(values),
    })
}

#[tokio::test]
async fn a_018_client_round_trips_platforms_without_losing_or_doubling_labels() {
    let d = spawn_daemon().await;
    let mut owner = WsClient::connect(&d).await;
    let project_id = common::peers::project(&mut owner, "The Hermes").await;
    call(
        &mut owner,
        Request::BoardGet(c::BoardGet {
            project_id: project_id.clone(),
        }),
    )
    .await;

    let item = edited(
        call(
            &mut owner,
            Request::ItemCreate(c::ItemCreate {
                project_id: project_id.clone(),
                r#type: c::ItemType::Bug as i32,
                title: "Crash".into(),
                labels: owned(&["urgent"]),
                platforms: vec![IOS],
                ..Default::default()
            }),
        )
        .await,
    );
    assert_eq!(item.labels, ["urgent", "platform:ios"]);
    assert_eq!(item.platforms, [IOS]);

    // Both at once: the labels it names, then its platforms in place of
    // the old ones.
    let item = update(
        &mut owner,
        &item,
        c::ItemUpdate {
            labels: labels(&["urgent", "later"]),
            platforms: platforms(&[DESKTOP]),
            ..Default::default()
        },
    )
    .await;
    assert_eq!(item.labels, ["urgent", "later", "platform:desktop"]);
    assert_eq!(item.platforms, [DESKTOP]);

    // The labels as it was sent them, platform labels included: none twice.
    let item = update(
        &mut owner,
        &item,
        c::ItemUpdate {
            labels: Some(c::StringList {
                values: item.labels.clone(),
            }),
            platforms: platforms(&[DESKTOP, DAEMON]),
            ..Default::default()
        },
    )
    .await;
    assert_eq!(
        item.labels,
        ["urgent", "later", "platform:desktop", "platform:daemon"]
    );

    // Neither: both stay.
    let item = update(
        &mut owner,
        &item,
        c::ItemUpdate {
            title: Some("Crash on launch".into()),
            ..Default::default()
        },
    )
    .await;
    assert_eq!(item.platforms, [DESKTOP, DAEMON]);
    assert_eq!(item.labels.len(), 4);

    // No platforms: only the platform labels go.
    let item = update(
        &mut owner,
        &item,
        c::ItemUpdate {
            platforms: platforms(&[]),
            ..Default::default()
        },
    )
    .await;
    assert_eq!(item.labels, ["urgent", "later"]);
    assert!(item.platforms.is_empty());

    // A 0.18 query by platform reads the labels.
    let item = update(
        &mut owner,
        &item,
        c::ItemUpdate {
            platforms: platforms(&[IOS]),
            ..Default::default()
        },
    )
    .await;
    let found = call(
        &mut owner,
        Request::ItemQuery(c::ItemQuery {
            project_id: project_id.clone(),
            platforms: vec![IOS],
            ..Default::default()
        }),
    )
    .await;
    let Response::Items(found) = response(found) else {
        panic!("expected cards");
    };
    assert_eq!(found.cards.len(), 1);
    assert_eq!(found.cards[0].id, item.id);
    assert_eq!(found.cards[0].platforms, [IOS]);
}
