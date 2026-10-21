//! A project whose board lives on a linked computer has its plan there too
//! (0.21.0): the mirroring computer refuses the plan's requests and says
//! where to go.

use crate::common;

use bus::contract::board::{self as c, board_request::Request};
use bus::contract::wire::envelope::Body;
use common::board::*;
use serde_json::json;

#[tokio::test]
async fn a_mirrored_projects_plan_is_refused_and_points_home() {
    let mut p = common::peers::paired().await;
    let mac_app = common::peers::project(&mut p.mac_client, "app").await;
    snapshot(
        call(
            &mut p.mac_client,
            Request::BoardGet(c::BoardGet {
                project_id: mac_app.clone(),
            }),
        )
        .await,
    );
    let win_app = common::peers::project(&mut p.win_client, "pc-app").await;
    let linked = p
        .win_client
        .request(json!({"type": "link_project", "project_id": win_app,
                        "peer_id": p.win_peer_id, "remote_project_id": mac_app}))
        .await;
    assert_eq!(linked["type"], "project", "{linked}");
    let win = &p.win;
    common::peers::wait_until("the PC mirrors the Mac's board", || {
        win.app.board_mirror.home_peer(&win_app).is_some()
    })
    .await;

    let requests = [
        Request::PlanGet(c::PlanGet {
            project_id: win_app.clone(),
        }),
        Request::PlanEnable(c::PlanEnable {
            project_id: win_app.clone(),
            enabled: true,
            research_gate: None,
        }),
        Request::PlanDocRead(c::PlanDocRead {
            project_id: win_app.clone(),
            path: "docs/guidelines/style.md".into(),
            ..Default::default()
        }),
    ];
    for request in requests {
        match call(&mut p.win_client, request).await {
            Body::Error(e) => {
                assert_eq!(e.code, "no_board", "{e:?}");
                assert!(e.message.contains("open it there"), "{e:?}");
            }
            other => panic!("expected a refusal, got {other:?}"),
        }
    }
}
