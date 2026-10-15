//! What a bot may do with its own card, and who leads a new project (0.19.0
//! card flow): the assignee edits, attaches, links and moves its card
//! under the column rules with no board role; a project's first bot leads
//! it; an assignment by the owner or the lead tells the bot, and every
//! notice carries the card's state.

use crate::common;

use bus::contract::board::{self as c, board_request::Request};
use common::board::{call, snapshot};
use common::cards::fresh;
use common::tasks::error_text;
use flotillad::kanban::model::Role;
use serde_json::json;

#[tokio::test]
async fn the_assignee_owns_its_card_without_a_board_role() {
    let (mut f, s) = fresh(&["Lead", "Helper"]).await;
    let helper_id = f.bot_ids[1].clone();
    assert!(
        !s.roles.iter().any(|r| r.bot_id == helper_id),
        "the second bot holds no role: {:?}",
        s.roles
    );
    let mine = f.item("Footer links").await;
    let other = f.item("Header logo").await;
    f.assign(&mine.id, Some("Helper")).await;

    // On its own card: edit, attach, link and move, with no role.
    let version = f.now(&mine.id).version;
    let edited = f.bots[1]
        .call(
            "item_update",
            json!({"id": mine.id, "expected_version": version,
                   "description": "Links to About and Contact."}),
        )
        .await;
    assert_eq!(
        edited["item"]["column_key"], "doing",
        "the item as it is now: {edited}"
    );
    assert!(
        edited["started"].is_string(),
        "its first edit starts it: {edited}"
    );
    assert_eq!(f.now(&mine.id).column_key, "doing");
    let attached = f.bots[1]
        .call(
            "item_attach",
            json!({"id": mine.id, "url": "https://example.com/footer"}),
        )
        .await;
    assert!(attached["attachment"].is_object(), "{attached}");
    let linked = f.bots[1]
        .call(
            "item_link",
            json!({"id": mine.id, "kind": "artifact", "ref": "footer.html"}),
        )
        .await;
    assert!(linked["link"].is_object(), "{linked}");
    let review = f.bot_move(1, &mine.id, "review").await;
    assert_ne!(review["isError"], json!(true), "{review}");
    // Done is the owner's, or a lead's who didn't do it.
    let done = f.bot_move(1, &mine.id, "done").await;
    assert!(
        error_text(&done).contains("Only the lead, the owner can move it to Done"),
        "{done}"
    );

    // On a card that isn't its own, the refusal says why, and that being
    // assigned is what it lacks.
    let raw = f.bots[1]
        .call_raw(
            "item_update",
            json!({"id": other.id, "expected_version": other.version, "title": "Logo"}),
        )
        .await;
    let text = error_text(&raw);
    assert!(text.contains("it isn't assigned to you"), "{text}");
    assert!(text.contains("Ask the lead to assign it to you"), "{text}");
    let raw = f.bots[1]
        .call_raw("item_create", json!({"type": "chore", "title": "Mine"}))
        .await;
    let text = error_text(&raw);
    assert!(
        text.contains("on a card assigned to you, you may use item_move"),
        "{text}"
    );
    assert!(!text.contains("or an item assigned to you"), "{text}");

    // The lead, who didn't do the work, may approve it.
    let approved = f.bot_move(0, &mine.id, "done").await;
    assert_ne!(approved["isError"], json!(true), "{approved}");
}

#[tokio::test]
async fn a_projects_first_bot_leads_it_even_when_the_board_came_first() {
    let d = common::spawn_daemon().await;
    let mut owner = common::WsClient::connect(&d).await;
    let project = common::peers::project(&mut owner, "Garden").await;
    let started = snapshot(
        call(
            &mut owner,
            Request::BoardGet(c::BoardGet {
                project_id: project.clone(),
            }),
        )
        .await,
    );
    assert!(started.roles.is_empty());
    let first = common::create_bot(&mut owner, &project, "First").await;
    let first_id = first["id"].as_str().expect("id").to_string();
    let second = common::create_bot(&mut owner, &project, "Second").await;
    let second_id = second["id"].as_str().expect("id").to_string();
    let p = d.app.db.get_project(&project).unwrap().expect("project");
    assert_eq!(p.lead_bot_id.as_deref(), Some(first_id.as_str()));
    let roles = d.app.db.project_roles(&project).unwrap();
    assert!(roles
        .iter()
        .any(|r| r.bot_id == first_id && r.role == Role::Lead));
    assert!(!roles.iter().any(|r| r.bot_id == second_id));

    // A project that has a lead keeps it.
    let other = common::peers::project(&mut owner, "Orchard").await;
    let a = common::create_bot(&mut owner, &other, "A").await;
    let b = common::create_bot(&mut owner, &other, "B").await;
    let b_id = b["id"].as_str().expect("id");
    let set = owner
        .request(json!({"type": "set_project_lead", "project_id": other, "bot_id": b_id}))
        .await;
    assert_ne!(set["type"], "error", "{set}");
    common::create_bot(&mut owner, &other, "C").await;
    let o = d.app.db.get_project(&other).unwrap().expect("project");
    assert_eq!(o.lead_bot_id.as_deref(), Some(b_id));
    assert_ne!(o.lead_bot_id.as_deref(), a["id"].as_str());
}

#[tokio::test]
async fn an_assignment_by_the_lead_tells_the_bot_and_an_unassignment_tells_it_too() {
    let (mut f, _) = fresh(&["Lead", "Helper"]).await;
    let item = f.item("Newsletter form").await;
    let assigned = f.bots[0]
        .call(
            "item_assign",
            json!({"id": item.id, "expected_version": item.version, "bot": "Helper"}),
        )
        .await;
    assert_eq!(assigned["item"]["assignee"], f.bot_ids[1].as_str());
    let told = f.heard(1, "assigned").await;
    assert!(
        told.contains(&format!(
            "Lead assigned {} “Newsletter form” to you.",
            item.id
        )),
        "{told}"
    );

    // The owner's edit is news, not work, and names the card's state.
    let version = f.now(&item.id).version;
    call(
        &mut f.owner,
        Request::ItemUpdate(c::ItemUpdate {
            id: item.id.clone(),
            expected_version: version,
            title: Some("Newsletter signup".into()),
            ..Default::default()
        }),
    )
    .await;
    // The edit right after the assignment folds into it; the unassignment
    // never folds.
    f.assign(&item.id, None).await;
    let off = f.heard(1, "off you").await;
    assert!(off.contains("it is no longer yours"), "{off}");

    // The lead's own change to its own card tells nobody.
    f.assign(&item.id, Some("Lead")).await;
    f.heard(0, "assigned").await;
    let version = f.now(&item.id).version;
    f.bots[0]
        .call(
            "item_update",
            json!({"id": item.id, "expected_version": version, "description": "Mine now."}),
        )
        .await;
    let quiet = common::tasks::drain_all(&mut f.bots[0]).await;
    assert!(
        !quiet
            .iter()
            .any(|m| m["body"].as_str().is_some_and(|b| b.contains("edited it"))),
        "{quiet:?}"
    );
}
