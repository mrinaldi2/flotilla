//! The plan over WebSocket (0.21.0, #43, `plan_v1`): its requests, the
//! `plan_change` push to watchers, cards that belong to a node, and its
//! documents read from the repository.

use crate::common;

use std::time::Duration;

use bus::contract::board::{
    self as c, board_push::Push, board_request::Request, board_response::Response,
};
use bus::contract::wire::envelope::Body;
use common::board::{call, error_code, next_envelope, response};
use common::plan::*;
use flotillad::plan::model::Stage;

/// The next `plan_change` push, skipping board events.
async fn plan_change(c: &mut common::WsClient) -> c::PlanChange {
    loop {
        let envelope = next_envelope(c, Duration::from_secs(3))
            .await
            .expect("a push");
        if let Some(Body::BoardPush(c::BoardPush {
            push: Some(Push::PlanChange(change)),
        })) = envelope.body
        {
            return change;
        }
    }
}

fn planned_node(r: Response) -> c::PlanNode {
    match r {
        Response::Planned(c::PlanResult {
            outcome: Some(c::plan_result::Outcome::Done(node)),
            ..
        }) => node,
        other => panic!("not done: {other:?}"),
    }
}

#[tokio::test]
async fn plan_requests_are_served_and_watchers_hear_each_change() {
    let mut plan = unplanned().await;
    let pid = plan.pid.clone();
    let Response::Plan(tree) = response(
        call(
            &mut plan.owner,
            Request::PlanEnable(c::PlanEnable {
                project_id: pid.clone(),
                enabled: true,
            }),
        )
        .await,
    ) else {
        panic!("a tree");
    };
    assert!(tree.enabled && tree.has_repo);
    assert_eq!(tree.guidelines[0].path, "docs/guidelines/style.md");

    let mut watcher = common::WsClient::connect(&plan.d).await;
    call(
        &mut watcher,
        Request::BoardWatch(c::BoardWatch {
            project_id: pid.clone(),
        }),
    )
    .await;
    let goal = planned_node(response(
        call(
            &mut plan.owner,
            Request::PlanCreate(c::PlanCreate {
                project_id: pid.clone(),
                kind: c::PlanKind::Goal as i32,
                title: "Ship the demo".into(),
                ..Default::default()
            }),
        )
        .await,
    ));
    let change = plan_change(&mut watcher).await;
    assert_eq!(change.project_id, pid);
    assert_eq!(
        change.node.as_ref().map(|n| n.id.as_str()),
        Some(goal.id.as_str())
    );
    assert_eq!(change.actor.as_deref(), Some("user"));

    let init = planned_node(response(
        call(
            &mut plan.owner,
            Request::PlanCreate(c::PlanCreate {
                project_id: pid.clone(),
                kind: c::PlanKind::Initiative as i32,
                title: "Combat readability".into(),
                parent_id: Some(goal.id.clone()),
                ..Default::default()
            }),
        )
        .await,
    ));
    let change = plan_change(&mut watcher).await;
    assert_eq!(change.node.unwrap().stage, c::PlanStage::Research as i32);

    // A card of the initiative, and one that names no node of the project.
    let create = |node: &str| {
        Request::ItemCreate(c::ItemCreate {
            project_id: pid.clone(),
            r#type: c::ItemType::Spike as i32,
            title: "Look around".into(),
            board_id: init.board_id.clone(),
            plan_node_id: Some(node.into()),
            ..Default::default()
        })
    };
    let Response::Edited(c::EditResult {
        outcome: Some(c::edit_result::Outcome::Refused(refused)),
        ..
    }) = response(call(&mut plan.owner, create("PL-99")).await)
    else {
        panic!("an unknown node is refused");
    };
    assert_eq!(refused.unmet[0].code, "plan.not_found");
    let Response::Edited(c::EditResult {
        outcome: Some(c::edit_result::Outcome::Done(item)),
        ..
    }) = response(call(&mut plan.owner, create(&init.id)).await)
    else {
        panic!("a card of the node");
    };
    assert_eq!(item.plan_node_id.as_deref(), Some(init.id.as_str()));
    let loose = plan
        .d
        .app
        .db
        .create_item(
            &flotillad::db::NewItem {
                project_id: &pid,
                board_id: None,
                item_type: flotillad::kanban::model::ItemType::Task,
                title: "No node",
                description: "",
                size: None,
                priority: flotillad::kanban::model::Priority::P2,
                labels: &[],
                parent_id: None,
                acceptance_criteria: &[],
                plan_node_id: None,
            },
            &flotillad::actor::Actor::User,
        )
        .unwrap();

    // A query for the goal finds its initiative's cards, and only them.
    let query = |nodes: Vec<String>| {
        Request::ItemQuery(c::ItemQuery {
            project_id: pid.clone(),
            plan_node_ids: nodes,
            ..Default::default()
        })
    };
    let Response::Items(found) =
        response(call(&mut plan.owner, query(vec![goal.id.clone()])).await)
    else {
        panic!("items");
    };
    // The lead's kickoff cards belong to the subtree too.
    let ids: Vec<&str> = found
        .cards
        .iter()
        .filter(|c| !c.title.starts_with("Research: ") && !c.title.starts_with("Propose "))
        .map(|c| c.id.as_str())
        .collect();
    assert_eq!(ids, [item.id.as_str()]);
    assert_eq!(
        found.cards[0].plan_node_id.as_deref(),
        Some(init.id.as_str())
    );
    let Response::Items(all) = response(call(&mut plan.owner, query(Vec::new())).await) else {
        panic!("items");
    };
    assert!(all.cards.iter().any(|c| c.id == loose.id));

    // The node's detail lists its card.
    let Response::PlanNode(detail) = response(
        call(
            &mut plan.owner,
            Request::PlanNodeGet(c::PlanNodeGet {
                id: init.id.clone(),
            }),
        )
        .await,
    ) else {
        panic!("a node");
    };
    assert_eq!(
        detail
            .cards
            .iter()
            .filter(|c| !c.labels.iter().any(|l| l == "kickoff"))
            .count(),
        1
    );
    assert_eq!(detail.history[0].kind, "created");
    let missing = call(
        &mut plan.owner,
        Request::PlanNodeGet(c::PlanNodeGet { id: "PL-99".into() }),
    )
    .await;
    assert_eq!(error_code(missing), "not_found");
}

#[tokio::test]
async fn documents_are_read_from_the_repository_with_their_diff() {
    let mut plan = planned().await;
    let pid = plan.pid.clone();
    let path = "docs/initiatives/combat/prd.md";
    let read = |sha: Option<String>, diff_from: Option<String>, offset: u64, limit: u32| {
        Request::PlanDocRead(c::PlanDocRead {
            project_id: pid.clone(),
            path: path.into(),
            sha,
            diff_from,
            offset,
            limit,
        })
    };
    let Response::PlanDoc(first) = response(call(&mut plan.owner, read(None, None, 0, 0)).await)
    else {
        panic!("a document");
    };
    assert_eq!(first.mime, "text/markdown");
    assert_eq!(first.data, b"# Combat readability\n\nFirst draft.\n");
    assert_eq!(first.total, first.data.len() as u64);
    assert!(first.diff.is_none());

    // Approved at that commit, the PRD then changes on main.
    let goal = plan.goal("Ship the demo");
    let init = plan.skip(&plan.initiative(&goal.id, "Combat readability"));
    let init = plan.submit(&init, path, &[]);
    assert_eq!(
        init.gate.as_ref().unwrap().sha.as_deref(),
        Some(first.sha.as_str())
    );
    let (init, _) = plan.approve(&init, &[]);
    assert_eq!(init.stage, Stage::Running);
    let detail = plan.owner().detail(&init.id).unwrap().unwrap();
    assert!(!detail.node.docs[0].changed_since_approval);
    let head = push_doc(&plan, path, "# Combat readability\n\nSecond draft.\n");
    let detail = plan.owner().detail(&init.id).unwrap().unwrap();
    assert!(
        detail.node.docs[0].changed_since_approval,
        "{:?}",
        detail.node.docs
    );

    let Response::PlanDoc(now) =
        response(call(&mut plan.owner, read(None, Some(first.sha.clone()), 0, 0)).await)
    else {
        panic!("a document");
    };
    assert_eq!(now.sha, head);
    let diff = now.diff.expect("a diff");
    assert!(
        diff.contains("-First draft.") && diff.contains("+Second draft."),
        "{diff}"
    );
    // A later chunk of the commit it was read at, without the diff.
    let Response::PlanDoc(chunk) = response(
        call(
            &mut plan.owner,
            read(Some(first.sha.clone()), Some(first.sha.clone()), 2, 6),
        )
        .await,
    ) else {
        panic!("a chunk");
    };
    assert_eq!((chunk.offset, chunk.data.as_slice()), (2, &b"Combat"[..]));
    assert!(chunk.diff.is_none());

    for (bad, code) in [
        ("../etc/passwd", "invalid_request"),
        ("docs/none.md", "not_found"),
        ("docs/initiatives", "invalid_request"),
    ] {
        let refused = call(
            &mut plan.owner,
            Request::PlanDocRead(c::PlanDocRead {
                project_id: pid.clone(),
                path: bad.into(),
                ..Default::default()
            }),
        )
        .await;
        assert_eq!(error_code(refused), code, "{bad}");
    }
}
