//! Workers from a role (0.23.0 step 3): the lead's extra hands copy the
//! role's member (its instructions, then the lead's, and its model), hold
//! the role until they retire, and wait in the queue with what they copy.
//! See `docs/workers.md`.

use crate::common;

use common::peers::{bot_named, wait_until};
use common::tasks::{error_text, project_with_bots_on};
use common::{spawn_daemon_with, McpClient, TestDaemon};
use flotillad::kanban::model::{ProjectRole, Role};
use serde_json::{json, Value};

const MODEL: &str = "claude-opus-5-5";

fn roles_of(d: &TestDaemon, project: &str, bot_id: &str) -> Vec<Role> {
    d.app
        .db
        .project_roles(project)
        .unwrap()
        .into_iter()
        .filter(|r| r.bot_id == bot_id)
        .map(|r| r.role)
        .collect()
}

fn worker_client(d: &TestDaemon, bot_id: &str) -> McpClient {
    let token = d.app.secrets.bot_token(bot_id).expect("token");
    McpClient::new(d, &token)
}

async fn spawn(lead: &mut McpClient, name: &str, item: &str) -> Value {
    lead.call(
        "spawn_worker",
        json!({"task": format!("Build {name}."), "name": name, "role": "dev",
               "instructions": "Only the parser.", "item": item}),
    )
    .await
}

#[tokio::test]
async fn a_worker_from_a_role_copies_its_member_and_drops_the_role_on_retiring() {
    let d = spawn_daemon_with(|cfg| cfg.max_workers_per_project = 1).await;
    let (pair, mut bots) = project_with_bots_on(d, &["Team Lead", "Dev A"]).await;
    let d = &pair.d;
    let dev = &pair.ids[1];
    let project = d.app.db.get_bot(dev).unwrap().unwrap().project_id;
    d.app
        .db
        .ensure_board(&project, &d.app.db.daemon_id().unwrap(), Some("P"))
        .unwrap();
    d.app
        .db
        .set_project_role(&ProjectRole {
            project_id: project.clone(),
            role: Role::Dev,
            bot_id: dev.clone(),
            machine: None,
        })
        .unwrap();
    d.app
        .db
        .update_bot(dev, None, None, Some("Write the tests first."), None)
        .unwrap();
    d.app.db.set_bot_model(dev, Some(MODEL)).unwrap();
    let card = bots[0]
        .call("item_create", json!({"type": "task", "title": "Parser"}))
        .await["item"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Only the lead spawns from a role, and only a role someone holds.
    let raw = bots[1]
        .call_raw(
            "spawn_worker",
            json!({"task": "x", "role": "dev", "item": card}),
        )
        .await;
    assert!(error_text(&raw).contains("only the lead"), "{raw}");
    let raw = bots[0]
        .call_raw(
            "spawn_worker",
            json!({"task": "x", "role": "architect", "item": card}),
        )
        .await;
    assert!(
        error_text(&raw).contains("no bot holds the architect role"),
        "{raw}"
    );
    let raw = bots[0]
        .call_raw(
            "spawn_worker",
            json!({"task": "x", "role": "devops", "item": card}),
        )
        .await;
    assert!(
        error_text(&raw).contains("can't copy the devops role"),
        "{raw}"
    );

    // The first starts at once; the second waits for the one slot.
    let lead = &mut bots[0];
    let first = spawn(lead, "dev-2", &card).await;
    assert_eq!(first["state"], "running", "{first}");
    let second = spawn(lead, "dev-3", &card).await;
    assert_eq!(second["state"], "queued", "{second}");

    let worker = bot_named(d, &project, "dev-2").expect("worker");
    assert!(worker.temporary);
    assert_eq!(
        worker.instructions,
        "Write the tests first.\n\nOnly the parser."
    );
    assert_eq!(
        d.app.db.bot_model(&worker.id).unwrap().as_deref(),
        Some(MODEL)
    );
    assert_eq!(roles_of(d, &project, &worker.id), [Role::Dev]);

    // It retires when its task closes, and its role goes with it.
    worker_client(d, &worker.id)
        .call(
            "complete_task",
            json!({"task_id": first["task_id"], "result": "parser built"}),
        )
        .await;
    wait_until("dev-2 retires", || {
        bot_named(d, &project, "dev-2").is_none()
    })
    .await;
    assert!(roles_of(d, &project, &worker.id).is_empty());

    // The queued one starts in the freed slot with what it copied.
    wait_until("dev-3 starts", || bot_named(d, &project, "dev-3").is_some()).await;
    let third = bot_named(d, &project, "dev-3").expect("worker");
    assert_eq!(
        d.app.db.bot_model(&third.id).unwrap().as_deref(),
        Some(MODEL)
    );
    assert_eq!(roles_of(d, &project, &third.id), [Role::Dev]);
    // The member keeps its role through all of it.
    assert_eq!(roles_of(d, &project, dev), [Role::Dev]);
}
