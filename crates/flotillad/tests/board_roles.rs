//! Board roles and the tools they list over MCP: a new board gives only the
//! project's lead a role, whatever the bots are called, and a bot without a
//! role still reads the board.

use crate::common;

use bus::contract::board::{self as c, board_request::Request};
use common::board::{call, snapshot};
use common::tasks::project_with_bots;
use common::WsClient;
use hermesd::kanban::model::TextValue;
use serde_json::{json, Value};

fn tool_names(list: &Value) -> Vec<String> {
    list["tools"]
        .as_array()
        .expect("tools")
        .iter()
        .map(|t| t["name"].as_str().expect("name").to_string())
        .collect()
}

const NAMES: &[&str] = &[
    "Team Lead",
    "Scrum Master",
    "devops",
    "Architect",
    "UX Designer",
    "Tester Win",
    "Desktop Dev",
    "iOS Dev",
    "Writer",
];

/// No role comes from a name (0.19.0 K8): "Team Lead" or "Tester Win" is
/// just a name, and the project's lead, here "Writer", leads.
#[tokio::test]
async fn a_new_board_gives_only_the_projects_lead_a_role() {
    let (pair, _bots) = project_with_bots(NAMES).await;
    let db = &pair.d.app.db;
    let project_id = db.get_bot(&pair.ids[0]).unwrap().unwrap().project_id;
    let writer = &pair.ids[NAMES.len() - 1];
    db.set_project_lead(&project_id, Some(writer)).unwrap();
    let mut owner = WsClient::connect(&pair.d).await;
    let board = snapshot(
        call(
            &mut owner,
            Request::BoardGet(c::BoardGet {
                project_id: project_id.clone(),
            }),
        )
        .await,
    );
    let roles: Vec<(String, String)> = db
        .project_roles(&project_id)
        .unwrap()
        .into_iter()
        .map(|r| (r.bot_id, r.role.as_text().to_string()))
        .collect();
    assert_eq!(roles, [(writer.clone(), "lead".to_string())]);
    assert_eq!(board.roles.len(), 1);
}

#[tokio::test]
async fn a_bot_without_a_role_lists_the_reads_and_its_own_items_tools() {
    let (pair, mut bots) = project_with_bots(&["Desktop Dev", "Writer"]).await;
    let db = &pair.d.app.db;
    let project_id = db.get_bot(&pair.ids[0]).unwrap().unwrap().project_id;
    db.ensure_board(&project_id, &db.daemon_id().unwrap(), Some("P"))
        .unwrap();
    crate::common::roles::by_name(db, &project_id);
    let [dev, writer] = &mut bots[..] else {
        unreachable!()
    };

    let mut reads = tool_names(&writer.tools().await);
    reads.retain(|n| n.starts_with("board_") || n.starts_with("item_"));
    reads.sort();
    // The reads, what it may do on an item it works on or was tasked
    // through (H-099), and on a draft it is invited to (0.19.0 K4b); every
    // other tool needs a role.
    assert_eq!(
        reads,
        [
            "board_get",
            "board_list",
            "item_attach",
            "item_comment",
            "item_detach",
            "item_get",
            "item_link",
            "item_move",
            "item_move_check",
            "item_query",
            "item_unlink",
            "item_update"
        ]
    );
    let board = writer.call("board_get", json!({})).await;
    assert!(board["columns"].is_array(), "{board}");
    let raw = writer
        .call_raw(
            "item_create",
            json!({"type": "chore", "title": "x", "platforms": ["daemon"]}),
        )
        .await;
    assert!(
        common::tasks::error_text(&raw).contains("board role"),
        "{raw}"
    );

    let dev_tools = tool_names(&dev.tools().await);
    assert!(dev_tools.contains(&"item_create".to_string()));
    assert!(dev_tools.contains(&"item_move".to_string()));
}
