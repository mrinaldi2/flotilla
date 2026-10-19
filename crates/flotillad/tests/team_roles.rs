//! The team by role (0.23.0 step 3): the lead sets Product and Architect,
//! assigns a card to a role and the daemon picks the holder (awake and free,
//! else asleep, else the first), and refuses a role nobody holds.

use crate::common;

use bus::contract::board::{self as c, board_request::Request, board_response::Response};
use bus::BotState;
use common::board::{call, response, version};
use common::peers::wait_until;
use common::tasks::{error_text, project_with_bots};
use common::{McpClient, TestDaemon, WsClient};
use flotillad::kanban::model::{ProjectRole, Role};
use serde_json::{json, Value};

fn project_of(d: &TestDaemon, bot_id: &str) -> String {
    d.app.db.get_bot(bot_id).unwrap().unwrap().project_id
}

fn give(d: &TestDaemon, project_id: &str, bot_id: &str, role: Role) {
    d.app
        .db
        .set_project_role(&ProjectRole {
            project_id: project_id.to_string(),
            role,
            bot_id: bot_id.to_string(),
            machine: None,
        })
        .unwrap();
}

async fn card(lead: &mut McpClient, title: &str) -> Value {
    lead.call("item_create", json!({"type": "task", "title": title}))
        .await["item"]
        .clone()
}

async fn assign_role(lead: &mut McpClient, item: &Value, role: &str) -> Value {
    lead.call_raw(
        "item_assign",
        json!({"id": item["id"], "expected_version": version(item), "role": role}),
    )
    .await
}

fn assignee(raw: &Value) -> String {
    let text = raw["content"][0]["text"]
        .as_str()
        .unwrap_or_else(|| panic!("{raw}"));
    let reply: Value = serde_json::from_str(text).expect("json");
    reply["item"]["assignee"]
        .as_str()
        .unwrap_or_else(|| panic!("no assignee: {reply}"))
        .to_string()
}

async fn ready(d: &TestDaemon, ids: &[&String]) {
    for id in ids {
        let app = d.app.clone();
        let id = (*id).clone();
        wait_until("the bot is up", move || {
            app.supervisor.state(&id).0 == BotState::Ready
        })
        .await;
    }
}

#[tokio::test]
async fn the_lead_sets_product_and_architect_but_never_reviewer_ce() {
    let (pair, mut bots) = project_with_bots(&["Team Lead", "Pat", "Ari"]).await;
    let d = &pair.d;
    let project = project_of(d, &pair.ids[0]);
    d.app
        .db
        .ensure_board(&project, &d.app.db.daemon_id().unwrap(), Some("P"))
        .unwrap();
    let lead = &mut bots[0];
    lead.call("role_set", json!({"bot": "Pat", "role": "product"}))
        .await;
    let set = lead
        .call("role_set", json!({"bot": "Ari", "role": "architect"}))
        .await;
    let roles: Vec<(String, String)> = set["roles"]
        .as_array()
        .expect("roles")
        .iter()
        .map(|r| {
            (
                r["bot_id"].as_str().unwrap().to_string(),
                r["role"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    assert!(
        roles.contains(&(pair.ids[1].clone(), "product".to_string())),
        "{set}"
    );
    assert!(
        roles.contains(&(pair.ids[2].clone(), "architect".to_string())),
        "{set}"
    );

    let raw = lead
        .call_raw("role_set", json!({"bot": "Ari", "role": "reviewer.ce"}))
        .await;
    assert!(error_text(&raw).contains("reviewer.ce"), "{raw}");
    // The lead takes a role away as it gives it.
    lead.call(
        "role_set",
        json!({"bot": "Pat", "role": "product", "remove": true}),
    )
    .await;
    assert!(!d
        .app
        .db
        .project_roles(&project)
        .unwrap()
        .iter()
        .any(|r| r.role == Role::Product));
}

/// Awake and free beats asleep, asleep beats busy, and with every holder
/// busy the first by name gets it. A role nobody holds is refused.
#[tokio::test]
async fn assigning_to_a_role_picks_the_readiest_holder() {
    let (pair, mut bots) = project_with_bots(&["Team Lead", "Dev A", "Dev B"]).await;
    let d = &pair.d;
    let (dev_a, dev_b) = (&pair.ids[1], &pair.ids[2]);
    let project = project_of(d, dev_a);
    d.app
        .db
        .ensure_board(&project, &d.app.db.daemon_id().unwrap(), Some("P"))
        .unwrap();
    give(d, &project, dev_a, Role::Dev);
    give(d, &project, dev_b, Role::Dev);
    ready(d, &[dev_a, dev_b]).await;
    let lead = &mut bots[0];

    // Dev A sorts first but sleeps; Dev B is awake and free.
    d.app.supervisor.set_state(dev_a, BotState::Asleep, "");
    let first = card(lead, "First").await;
    assert_eq!(assignee(&assign_role(lead, &first, "dev").await), *dev_b);

    // Dev B now holds a task: the sleeping Dev A gets the next card.
    lead.call(
        "send_message",
        json!({"to": "Dev B", "kind": "task", "body": "build it", "item": first["id"]}),
    )
    .await;
    let second = card(lead, "Second").await;
    assert_eq!(assignee(&assign_role(lead, &second, "dev").await), *dev_a);

    // Both busy: the first holder by name, whose work waits.
    d.app.supervisor.set_state(dev_a, BotState::Working, "");
    let third = card(lead, "Third").await;
    assert_eq!(assignee(&assign_role(lead, &third, "dev").await), *dev_a);

    // Nobody holds Architect.
    let fourth = card(lead, "Fourth").await;
    let raw = assign_role(lead, &fourth, "architect").await;
    assert!(error_text(&raw).contains("role.unheld"), "{raw}");
    // A bot without the lead's tools is refused for that, not the role.
    let raw = bots[1]
        .call_raw(
            "item_assign",
            json!({"id": fourth["id"], "expected_version": version(&fourth), "role": "architect"}),
        )
        .await;
    assert!(!error_text(&raw).contains("role.unheld"), "{raw}");
}

/// The owner assigns by role over the WebSocket too, and the holder is told.
#[tokio::test]
async fn the_owner_assigns_a_card_to_a_role() {
    let (pair, mut bots) = project_with_bots(&["Team Lead", "Pat"]).await;
    let d = &pair.d;
    let pat = &pair.ids[1];
    let project = project_of(d, pat);
    d.app
        .db
        .ensure_board(&project, &d.app.db.daemon_id().unwrap(), Some("P"))
        .unwrap();
    let item = card(&mut bots[0], "Shape the import").await;
    let mut owner = WsClient::connect(d).await;
    let assign = |role: c::Role| {
        Request::ItemAssign(c::ItemAssign {
            id: item["id"].as_str().unwrap().to_string(),
            expected_version: version(&item),
            bot: None,
            role: Some(role as i32),
        })
    };
    let Response::Edited(c::EditResult {
        outcome: Some(c::edit_result::Outcome::Refused(refused)),
        ..
    }) = response(call(&mut owner, assign(c::Role::Product)).await)
    else {
        panic!("expected a refusal");
    };
    assert_eq!(refused.unmet[0].code, "role.unheld");

    give(d, &project, pat, Role::Product);
    let Response::Edited(c::EditResult {
        outcome: Some(c::edit_result::Outcome::Done(done)),
        ..
    }) = response(call(&mut owner, assign(c::Role::Product)).await)
    else {
        panic!("expected the card assigned");
    };
    assert_eq!(done.assignee.as_deref(), Some(pat.as_str()));
    let told = common::tasks::drain_until(&mut bots[1], "to you. It is in").await;
    assert!(!told.is_empty());
}
