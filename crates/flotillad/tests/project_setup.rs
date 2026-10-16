//! Setting a project up from the phone (0.20.0): what it is for
//! (`project_description`) and a lead from the start (`create_bot_lead`).

use crate::common;

use bus::contract::board::{self as c, board_request::Request};
use common::board::*;
use common::*;
use flotillad::kanban::model::Role;
use serde_json::{json, Value};

async fn bot(owner: &mut WsClient, project_id: &str, name: &str, lead: Option<bool>) -> Value {
    let mut req = json!({"type": "create_bot", "project_id": project_id, "name": name});
    if let Some(lead) = lead {
        req["lead"] = json!(lead);
    }
    let created = owner.request(req).await;
    assert_eq!(created["type"], "bot", "{created}");
    created["bot"].clone()
}

fn leads(d: &TestDaemon, project_id: &str) -> Vec<String> {
    d.app
        .db
        .project_roles(project_id)
        .unwrap()
        .into_iter()
        .filter(|r| r.role == Role::Lead)
        .map(|r| r.bot_id)
        .collect()
}

#[tokio::test]
async fn a_project_carries_its_description() {
    let d = spawn_daemon().await;
    let mut owner = WsClient::connect(&d).await;
    let created = owner
        .request(json!({"type": "create_project", "name": "Recipes",
                        "description": "  A recipe site for my family.\n"}))
        .await;
    assert_eq!(
        created["project"]["description"],
        "A recipe site for my family."
    );
    let id = created["project"]["id"].as_str().unwrap().to_string();
    let plain = owner
        .request(json!({"type": "create_project", "name": "Plain"}))
        .await;
    assert_eq!(plain["project"]["description"], "");

    let listed = owner.request(json!({"type": "list_projects"})).await;
    let mine = listed["projects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == id.as_str())
        .unwrap()
        .clone();
    assert_eq!(mine["description"], "A recipe site for my family.");

    // Either field alone, or both.
    let described = owner
        .request(json!({"type": "update_project", "project_id": id,
                        "description": "Recipes, with photos."}))
        .await;
    assert_eq!(described["project"]["name"], "Recipes");
    assert_eq!(described["project"]["description"], "Recipes, with photos.");
    let renamed = owner
        .request(json!({"type": "update_project", "project_id": id, "name": "Kitchen"}))
        .await;
    assert_eq!(renamed["project"]["name"], "Kitchen");
    assert_eq!(renamed["project"]["description"], "Recipes, with photos.");
    let cleared = owner
        .request(json!({"type": "update_project", "project_id": id, "description": ""}))
        .await;
    assert_eq!(cleared["project"]["description"], "");
}

#[tokio::test]
async fn a_bad_description_is_refused() {
    let d = spawn_daemon().await;
    let mut owner = WsClient::connect(&d).await;
    let long = "x".repeat(2049);
    let refused = owner
        .request(json!({"type": "create_project", "name": "Long", "description": long}))
        .await;
    assert_eq!(refused["code"], "invalid_request", "{refused}");
    assert!(d.app.db.list_projects().unwrap().is_empty());

    let id = common::peers::project(&mut owner, "Short").await;
    for req in [
        json!({"type": "update_project", "project_id": id}),
        json!({"type": "update_project", "project_id": id, "description": 7}),
        json!({"type": "update_project", "project_id": id, "description": long}),
        // A refused description leaves the name alone too.
        json!({"type": "update_project", "project_id": id, "name": "Other",
               "description": long}),
    ] {
        let refused = owner.request(req).await;
        assert_eq!(refused["code"], "invalid_request", "{refused}");
    }
    let project = d.app.db.get_project(&id).unwrap().unwrap();
    assert_eq!(project.name, "Short");
}

/// Its bots are told what the project is for, and told again when it
/// changes.
#[tokio::test]
async fn bots_are_told_the_description() {
    let d = spawn_daemon().await;
    let mut owner = WsClient::connect(&d).await;
    let created = owner
        .request(json!({"type": "create_project", "name": "Told",
                        "description": "A tiny weather app."}))
        .await;
    let id = created["project"]["id"].as_str().unwrap().to_string();
    let bot = bot(&mut owner, &id, "Ada", None).await;
    let project = d.app.db.get_project(&id).unwrap().unwrap();
    let stored = d
        .app
        .db
        .get_bot(bot["id"].as_str().unwrap())
        .unwrap()
        .unwrap();
    let system = flotillad::paths::bot_dir(&d.app.cfg, &project.dir_name, &stored.dir_name)
        .join("system.md");
    let read = || std::fs::read_to_string(&system).unwrap();
    assert!(read().contains("A tiny weather app."));
    owner
        .request(json!({"type": "update_project", "project_id": id,
                        "description": "A weather app with alerts."}))
        .await;
    assert!(read().contains("A weather app with alerts."));
}

#[tokio::test]
async fn a_bot_made_lead_leads_the_project_and_its_board() {
    let d = spawn_daemon().await;
    let mut owner = WsClient::connect(&d).await;
    let id = common::peers::project(&mut owner, "Led").await;
    let first = bot(&mut owner, &id, "First", None).await;
    let first = first["id"].as_str().unwrap().to_string();
    snapshot(
        call(
            &mut owner,
            Request::BoardEnable(c::BoardEnable {
                project_id: id.clone(),
                ..Default::default()
            }),
        )
        .await,
    );
    // The first bot leads by default (0.19.0).
    assert_eq!(leads(&d, &id), std::slice::from_ref(&first));

    let lead = bot(&mut owner, &id, "Lead", Some(true)).await;
    let lead = lead["id"].as_str().unwrap().to_string();
    let project = d.app.db.get_project(&id).unwrap().unwrap();
    assert_eq!(project.lead_bot_id.as_deref(), Some(lead.as_str()));
    assert_eq!(leads(&d, &id), std::slice::from_ref(&lead));

    // `lead: false` changes nothing.
    bot(&mut owner, &id, "Other", Some(false)).await;
    assert_eq!(leads(&d, &id), [lead]);
}

#[tokio::test]
async fn a_lead_before_the_board_leads_the_board_it_starts() {
    let d = spawn_daemon().await;
    let mut owner = WsClient::connect(&d).await;
    let id = common::peers::project(&mut owner, "Early").await;
    bot(&mut owner, &id, "First", None).await;
    let lead = bot(&mut owner, &id, "Lead", Some(true)).await;
    let lead = lead["id"].as_str().unwrap().to_string();
    let project = d.app.db.get_project(&id).unwrap().unwrap();
    assert_eq!(project.lead_bot_id.as_deref(), Some(lead.as_str()));
    assert!(leads(&d, &id).is_empty(), "no board, no role");
    snapshot(
        call(
            &mut owner,
            Request::BoardEnable(c::BoardEnable {
                project_id: id.clone(),
                ..Default::default()
            }),
        )
        .await,
    );
    assert_eq!(leads(&d, &id), [lead]);

    let refused = owner
        .request(json!({"type": "create_bot", "project_id": id, "name": "Odd", "lead": "yes"}))
        .await;
    assert_eq!(refused["code"], "invalid_request", "{refused}");
}
