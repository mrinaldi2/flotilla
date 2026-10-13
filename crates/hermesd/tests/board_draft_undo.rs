//! The owner's review of a draft (0.19.0 K5): each of a bot's changes can
//! be undone on its own, and the owner's snapshot carries the lead's
//! proposals for Needs you.

use crate::common;

use bus::contract::board::{self as c, board_request::Request, board_response::Response};
use common::board::*;
use common::tasks::project_with_bots;
use common::{McpClient, WsClient};
use hermesd::kanban::admin::{self, By, Structure};
use hermesd::kanban::model::Role;
use hermesd::kanban::presets::Preset;
use serde_json::json;

fn edited(body: bus::contract::wire::envelope::Body) -> c::edit_result::Outcome {
    match response(body) {
        Response::Edited(c::EditResult {
            outcome: Some(outcome),
            ..
        }) => outcome,
        other => panic!("expected an edit result, got {other:?}"),
    }
}

fn done(body: bus::contract::wire::envelope::Body) -> c::Item {
    match edited(body) {
        c::edit_result::Outcome::Done(item) => item,
        other => panic!("expected the edit done, got {other:?}"),
    }
}

async fn revisions(owner: &mut WsClient, id: &str) -> Vec<c::DraftRevision> {
    let reply = call(
        owner,
        Request::ItemDraftChanges(c::ItemDraftChanges {
            id: id.into(),
            since: None,
            mark_seen: None,
        }),
    )
    .await;
    match response(reply) {
        Response::DraftChanges(changes) => changes.revisions,
        other => panic!("expected draft changes, got {other:?}"),
    }
}

async fn undo(owner: &mut WsClient, id: &str, version: u64, revision: i64) -> c::Item {
    done(
        call(
            owner,
            Request::ItemDraftUndo(c::ItemDraftUndo {
                id: id.into(),
                expected_version: version,
                revision,
            }),
        )
        .await,
    )
}

async fn item(owner: &mut WsClient, id: &str) -> c::ItemDetail {
    match response(call(owner, Request::ItemGet(c::ItemGet { id: id.into() })).await) {
        Response::Item(detail) => detail,
        other => panic!("expected an item, got {other:?}"),
    }
}

fn revision_of(revisions: &[c::DraftRevision], field: &str) -> i64 {
    revisions
        .iter()
        .rev()
        .find(|r| r.field == field)
        .unwrap_or_else(|| panic!("a {field} revision in {revisions:?}"))
        .id
}

#[tokio::test]
async fn the_owner_undoes_a_bots_changes_to_a_draft_one_at_a_time() {
    let (pair, mut bots) = project_with_bots(&["Lead"]).await;
    let lead: &mut McpClient = &mut bots[0];
    let d = &pair.d;
    let db = &d.app.db;
    let lead_id = pair.ids[0].clone();
    let project_id = db.get_bot(&lead_id).unwrap().unwrap().project_id;
    db.set_project_lead(&project_id, Some(&lead_id)).unwrap();
    db.ensure_board(&project_id, &db.daemon_id().unwrap(), Some("H"))
        .unwrap();
    crate::common::roles::by_name(db, &project_id);
    let recipes = db
        .create_board(&project_id, "Recipes", "RB", Preset::Simple)
        .unwrap();
    let mut owner = WsClient::connect(d).await;
    let draft = done(
        call(
            &mut owner,
            Request::ItemCreate(c::ItemCreate {
                project_id: project_id.clone(),
                r#type: c::ItemType::Feature as i32,
                title: "Unit conversion".into(),
                board_id: Some(recipes.id.clone()),
                draft: Some(true),
                labels: vec!["kitchen".into()],
                ..Default::default()
            }),
        )
        .await,
    );
    let id = draft.id.clone();
    call(
        &mut owner,
        Request::ItemInvite(c::ItemInvite {
            id: id.clone(),
            ..Default::default()
        }),
    )
    .await;

    // The lead retitles it, adds criteria and labels, a doc and a link.
    lead.call(
        "item_update",
        json!({"id": id, "expected_version": draft.version, "title": "Convert cups and grams",
               "labels": ["kitchen", "math"],
               "acceptance_criteria": ["Cups convert to grams"]}),
    )
    .await;
    let project = db.get_project(&project_id).unwrap().unwrap();
    let artifacts = hermesd::paths::artifacts_dir(&d.app.cfg, &project.dir_name);
    std::fs::create_dir_all(&artifacts).unwrap();
    std::fs::write(artifacts.join("spec.md"), "# Spec\n").unwrap();
    lead.call("item_attach", json!({"id": id, "artifact": "spec.md"}))
        .await;
    lead.call(
        "item_link",
        json!({"id": id, "kind": "artifact", "ref": "notes.md"}),
    )
    .await;
    let changes = revisions(&mut owner, &id).await;

    // Each change comes back on its own: the title, and nothing else.
    let version = item(&mut owner, &id).await.item.unwrap().version;
    let back = undo(&mut owner, &id, version, revision_of(&changes, "title")).await;
    assert_eq!(back.title, "Unit conversion");
    assert_eq!(back.labels, ["kitchen", "math"]);
    let back = undo(
        &mut owner,
        &id,
        back.version,
        revision_of(&changes, "labels"),
    )
    .await;
    assert_eq!(back.labels, ["kitchen"]);
    let back = undo(
        &mut owner,
        &id,
        back.version,
        revision_of(&changes, "acceptance_criteria"),
    )
    .await;
    assert!(back.acceptance_criteria.is_empty());

    // The doc it attached is detached; undoing that attaches it again.
    let back = undo(
        &mut owner,
        &id,
        back.version,
        revision_of(&changes, "attachment"),
    )
    .await;
    let detail = item(&mut owner, &id).await;
    assert!(detail.attachments.iter().all(|a| a.removed));
    let now = revisions(&mut owner, &id).await;
    let detach = now.last().expect("the undo's own revision");
    assert_eq!(
        (detach.actor.as_str(), detach.field.as_str()),
        ("user", "attachment")
    );
    undo(&mut owner, &id, back.version, detach.id).await;
    let detail = item(&mut owner, &id).await;
    assert!(detail.attachments.iter().any(|a| !a.removed));

    // The link it added is removed.
    let version = detail.item.unwrap().version;
    undo(&mut owner, &id, version, revision_of(&changes, "link")).await;
    let detail = item(&mut owner, &id).await;
    assert!(!detail.links.iter().any(|l| l.r#ref == "notes.md"));

    // A stale version is a conflict; once live, nothing is undone.
    let current = detail.item.unwrap();
    let stale = edited(
        call(
            &mut owner,
            Request::ItemDraftUndo(c::ItemDraftUndo {
                id: id.clone(),
                expected_version: current.version - 1,
                revision: revision_of(&changes, "title"),
            }),
        )
        .await,
    );
    assert!(matches!(stale, c::edit_result::Outcome::Conflict(_)));
    call(
        &mut owner,
        Request::ItemFinalize(c::ItemFinalize {
            id: id.clone(),
            expected_version: current.version,
            column: None,
        }),
    )
    .await;
    let live = db.get_item(&id).unwrap().unwrap();
    let refused = edited(
        call(
            &mut owner,
            Request::ItemDraftUndo(c::ItemDraftUndo {
                id: id.clone(),
                expected_version: live.version,
                revision: revision_of(&changes, "title"),
            }),
        )
        .await,
    );
    assert!(matches!(refused, c::edit_result::Outcome::Refused(_)));
}

#[tokio::test]
async fn the_owners_snapshot_carries_the_leads_open_proposal() {
    let d = common::spawn_daemon().await;
    let db = &d.app.db;
    let p = db.create_project("Recipe Box", "recipe-box").unwrap();
    let lead = db
        .create_bot(&p.id, "Lead", "", "", "", "/tmp/lead", "lead", None)
        .unwrap();
    db.set_project_lead(&p.id, Some(&lead.id)).unwrap();
    db.ensure_board(&p.id, &db.daemon_id().unwrap(), None)
        .unwrap();
    crate::common::roles::by_name(db, &p.id);
    let rs = db
        .create_board(&p.id, "Research", "RS", Preset::Software)
        .unwrap();
    let qa = Structure::ColumnAdd(c::ColumnAdd {
        board_id: rs.id.clone(),
        version: rs.version,
        name: "QA".into(),
        kind: c::ColumnKind::Active as i32,
        rules: vec!["assignee_set".into()],
        ..Default::default()
    });
    let by = By::Bot {
        bot: &lead,
        roles: &[Role::Lead],
    };
    admin::structure(&d.app, by, qa).expect("proposed");
    let mut owner = WsClient::connect(&d).await;
    let s = snapshot(
        call(
            &mut owner,
            Request::BoardGet(c::BoardGet {
                project_id: p.id.clone(),
            }),
        )
        .await,
    );
    let [proposal] = s.proposals.as_slice() else {
        panic!("one proposal in {:?}", s.proposals);
    };
    assert_eq!(proposal.state, "open");
    assert_eq!(proposal.board_id, rs.id);
    assert_eq!(proposal.proposed_by, format!("bot:{}", lead.id));
    assert!(!proposal.decision_id.is_empty());
    let Some(c::board_proposal::Change::ColumnAdd(add)) = &proposal.change else {
        panic!("a column_add change: {proposal:?}");
    };
    assert_eq!(
        (add.name.as_str(), add.rules.as_slice()),
        ("QA", &["assignee_set".to_string()][..])
    );
}

#[tokio::test]
async fn undoing_an_attachment_takes_back_that_one_not_another_of_its_name() {
    let (pair, mut bots) = project_with_bots(&["Lead"]).await;
    let lead: &mut McpClient = &mut bots[0];
    let d = &pair.d;
    let db = &d.app.db;
    let project_id = db.get_bot(&pair.ids[0]).unwrap().unwrap().project_id;
    db.ensure_board(&project_id, &db.daemon_id().unwrap(), Some("H"))
        .unwrap();
    let mut owner = WsClient::connect(d).await;
    let draft = done(
        call(
            &mut owner,
            Request::ItemCreate(c::ItemCreate {
                project_id: project_id.clone(),
                r#type: c::ItemType::Feature as i32,
                title: "Screenshots".into(),
                draft: Some(true),
                ..Default::default()
            }),
        )
        .await,
    );
    let id = draft.id.clone();
    call(
        &mut owner,
        Request::ItemInvite(c::ItemInvite {
            id: id.clone(),
            bot: Some(pair.ids[0].clone()),
            ..Default::default()
        }),
    )
    .await;
    let project = db.get_project(&project_id).unwrap().unwrap();
    let artifacts = hermesd::paths::artifacts_dir(&d.app.cfg, &project.dir_name);
    std::fs::create_dir_all(artifacts.join("v2")).unwrap();
    std::fs::write(artifacts.join("spec.md"), "# Spec\n").unwrap();
    std::fs::write(artifacts.join("v2/spec.md"), "# Spec, again\n").unwrap();
    // The lead attaches spec.md; then the owner attaches another, of that name.
    lead.call("item_attach", json!({"id": id, "artifact": "spec.md"}))
        .await;
    let by_lead = revision_of(&revisions(&mut owner, &id).await, "attachment");
    call(
        &mut owner,
        Request::ItemAttach(c::ItemAttach {
            id: id.clone(),
            artifact: Some("v2/spec.md".into()),
            name: Some("spec.md".into()),
            ..Default::default()
        }),
    )
    .await;
    let before = item(&mut owner, &id).await;
    let leads = |d: &c::ItemDetail| {
        d.attachments
            .iter()
            .find(|a| a.added_by.starts_with("bot:"))
            .cloned()
            .expect("the lead's attachment")
    };
    let owners = |d: &c::ItemDetail| {
        d.attachments
            .iter()
            .find(|a| a.added_by == "user")
            .cloned()
            .expect("the owner's attachment")
    };
    assert_eq!(
        (leads(&before).name.as_str(), owners(&before).name.as_str()),
        ("spec.md", "spec.md")
    );

    // Undoing the lead's change detaches the lead's file, not the owner's.
    let version = before.item.unwrap().version;
    undo(&mut owner, &id, version, by_lead).await;
    let after = item(&mut owner, &id).await;
    assert!(leads(&after).removed, "{:?}", after.attachments);
    assert!(!owners(&after).removed, "{:?}", after.attachments);
}

#[tokio::test]
async fn an_undo_of_no_such_revision_is_refused() {
    let d = common::spawn_daemon().await;
    let db = &d.app.db;
    let p = db.create_project("Recipe Box", "recipe-box").unwrap();
    db.ensure_board(&p.id, &db.daemon_id().unwrap(), None)
        .unwrap();
    let mut owner = WsClient::connect(&d).await;
    let draft = done(
        call(
            &mut owner,
            Request::ItemCreate(c::ItemCreate {
                project_id: p.id.clone(),
                r#type: c::ItemType::Feature as i32,
                title: "Unit conversion".into(),
                draft: Some(true),
                ..Default::default()
            }),
        )
        .await,
    );
    for revision in [0, -1, i64::MIN, i64::MAX] {
        let reply = call(
            &mut owner,
            Request::ItemDraftUndo(c::ItemDraftUndo {
                id: draft.id.clone(),
                expected_version: draft.version,
                revision,
            }),
        )
        .await;
        assert_eq!(error_code(reply), "not_found", "revision {revision}");
    }
}
