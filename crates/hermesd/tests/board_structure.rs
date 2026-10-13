//! Boards and their structure (0.19.0 K4): the owner over WebSocket, and a
//! lead whose changes apply or wait for the owner as the board's policy says.

use crate::common;

use bus::contract::board::{
    self as c, board_request::Request, board_response::Response, structure_result::Outcome as W,
};
use common::board::*;
use common::*;
use hermesd::actor::Actor;
use hermesd::kanban::admin::{self, By, Outcome, Structure};
use hermesd::kanban::model::Role;
use hermesd::kanban::presets::Preset;

fn applied(body: bus::contract::wire::envelope::Body) -> c::BoardSnapshot {
    match response(body) {
        Response::Structure(c::StructureResult {
            outcome: Some(W::Applied(s)),
            ..
        }) => s,
        other => panic!("expected an applied change, got {other:?}"),
    }
}

fn board_named<'a>(s: &'a c::BoardSnapshot, name: &str) -> &'a c::Board {
    s.boards.iter().find(|b| b.name == name).expect(name)
}

#[tokio::test]
async fn the_owner_shapes_a_custom_board_over_websocket() {
    let d = spawn_daemon().await;
    let mut owner = WsClient::connect(&d).await;
    let project_id = common::peers::project(&mut owner, "Recipe Box").await;
    let first = snapshot(
        call(
            &mut owner,
            Request::BoardGet(c::BoardGet {
                project_id: project_id.clone(),
            }),
        )
        .await,
    );
    assert_eq!(first.boards.len(), 1);
    assert!(first.boards[0].is_default);

    let s = applied(
        call(
            &mut owner,
            Request::BoardCreate(c::BoardCreate {
                project_id: project_id.clone(),
                name: "Research".into(),
                key: Some("RS".into()),
                preset: "software".into(),
                description: None,
            }),
        )
        .await,
    );
    let research = board_named(&s, "Research").clone();
    assert_eq!(research.flow, "custom");
    assert_eq!(research.automations["pr.opened"], "rs-review");

    // A column with its own rules, after Review.
    let s = applied(
        call(
            &mut owner,
            Request::ColumnAdd(c::ColumnAdd {
                board_id: research.id.clone(),
                version: research.version,
                name: "QA".into(),
                kind: qa_kind(),
                after: Some("rs-review".into()),
                movers: vec!["roles:tester".into()],
                rules: vec!["ac_all_checked".into()],
                ..Default::default()
            }),
        )
        .await,
    );
    let mine: Vec<&c::BoardColumn> = s
        .columns
        .iter()
        .filter(|col| col.board_id == research.id)
        .collect();
    let keys: Vec<&str> = mine.iter().map(|col| col.key.as_str()).collect();
    assert_eq!(
        keys,
        [
            "rs-inbox",
            "rs-ready",
            "rs-doing",
            "rs-review",
            "qa",
            "rs-done",
            "rs-cancelled"
        ]
    );
    let qa = mine.iter().find(|col| col.key == "qa").expect("qa");
    assert_eq!(qa.rules, ["ac_all_checked"]);
    assert_eq!(qa.movers, ["roles:tester"]);

    // A stale version is refused; a typo in a rule too.
    let stale = call(
        &mut owner,
        Request::BoardUpdate(c::BoardUpdate {
            board_id: research.id.clone(),
            version: research.version,
            name: Some("Lab".into()),
            ..Default::default()
        }),
    )
    .await;
    assert_eq!(error_code(stale), "conflict");
    let current = board_named(&s, "Research").clone();
    let typo = call(
        &mut owner,
        Request::ColumnUpdate(c::ColumnUpdate {
            board_id: current.id.clone(),
            version: current.version,
            key: "qa".into(),
            set_policy: Some(true),
            rules: vec!["link:prs".into()],
            ..Default::default()
        }),
    )
    .await;
    assert_eq!(error_code(typo), "invalid_request");

    // The release-gated board can be renamed, not restructured.
    let main = board_named(&s, "Main").clone();
    let refused = call(
        &mut owner,
        Request::ColumnAdd(c::ColumnAdd {
            board_id: main.id.clone(),
            version: main.version,
            name: "Extra".into(),
            kind: qa_kind(),
            ..Default::default()
        }),
    )
    .await;
    assert_eq!(error_code(refused), "invalid_request");
    let s = applied(
        call(
            &mut owner,
            Request::BoardUpdate(c::BoardUpdate {
                board_id: main.id.clone(),
                version: main.version,
                name: Some("Product".into()),
                ..Default::default()
            }),
        )
        .await,
    );
    assert!(s.boards.iter().any(|b| b.name == "Product"));
}

fn qa_kind() -> i32 {
    c::ColumnKind::Active as i32
}

#[tokio::test]
async fn a_leads_change_waits_for_the_owner_unless_the_board_is_lead_free() {
    let d = spawn_daemon().await;
    let db = &d.app.db;
    let p = db
        .create_project("Recipe Box", "recipe-box")
        .expect("project");
    let lead = db
        .create_bot(&p.id, "Lead", "", "", "", "/tmp/lead", "lead", None)
        .expect("lead");
    let dev = db
        .create_bot(&p.id, "Builder", "", "", "", "/tmp/dev", "builder", None)
        .expect("dev");
    db.set_project_lead(&p.id, Some(&lead.id)).expect("lead");
    db.ensure_board(&p.id, &db.daemon_id().expect("id"), None)
        .expect("board");
    crate::common::roles::by_name(db, &p.id);
    let rs = db
        .create_board(&p.id, "Research", "RS", Preset::Software)
        .expect("board");
    let add = |version: u64, name: &str| {
        Structure::ColumnAdd(c::ColumnAdd {
            board_id: rs.id.clone(),
            version,
            name: name.into(),
            kind: qa_kind(),
            ..Default::default()
        })
    };
    let as_lead = || By::Bot {
        bot: &lead,
        roles: &[Role::Lead],
    };

    // owner_confirms: proposed, with a decision for the owner.
    let Outcome::Proposed { proposal_id, .. } =
        admin::structure(&d.app, as_lead(), add(rs.version, "QA")).expect("proposed")
    else {
        panic!("expected a proposal");
    };
    let proposal = db.board_proposal(&proposal_id).expect("read").expect("row");
    assert_eq!(proposal.state, "open");
    let decision = proposal.decision_id.clone().expect("a decision");
    assert!(db.get_decision(&decision).expect("read").is_some());
    let has_qa = || {
        db.board_columns(&p.id)
            .expect("columns")
            .iter()
            .any(|col| col.key == "qa")
    };
    assert!(!has_qa(), "nothing changes before the owner rules");
    admin::rule(&d.app, &Actor::User, &proposal_id, true, None).expect("approved");
    assert!(has_qa());
    assert_eq!(
        db.board_proposal(&proposal_id)
            .expect("read")
            .expect("row")
            .state,
        "applied"
    );

    // A cosmetic change applies directly and is recorded.
    let rs_now = db.board(&rs.id).expect("read").expect("board");
    let renamed = admin::structure(
        &d.app,
        as_lead(),
        Structure::Board(c::BoardUpdate {
            board_id: rs.id.clone(),
            version: rs_now.version,
            name: Some("Lab".into()),
            ..Default::default()
        }),
    )
    .expect("renamed");
    assert!(matches!(renamed, Outcome::Applied { .. }));

    // Owner-only settings and other bots are refused.
    let rs_now = db.board(&rs.id).expect("read").expect("board");
    let policy = Structure::Board(c::BoardUpdate {
        board_id: rs.id.clone(),
        version: rs_now.version,
        structure_policy: Some("lead_free".into()),
        ..Default::default()
    });
    assert!(admin::structure(&d.app, as_lead(), policy.clone()).is_err());
    let as_dev = By::Bot {
        bot: &dev,
        roles: &[Role::Dev],
    };
    assert!(admin::structure(&d.app, as_dev, add(rs_now.version, "Docs")).is_err());

    // lead_free: the lead's change applies directly.
    admin::structure(&d.app, By::Owner(Actor::User), policy).expect("owner sets it");
    let rs_now = db.board(&rs.id).expect("read").expect("board");
    let direct = admin::structure(&d.app, as_lead(), add(rs_now.version, "Docs")).expect("applied");
    assert!(matches!(direct, Outcome::Applied { .. }));
    let changes = db.board_proposals(&p.id).expect("list");
    assert_eq!(
        changes.iter().filter(|c| c.state == "applied").count(),
        3,
        "{changes:?}"
    );

    // A declined proposal changes nothing.
    admin::structure(
        &d.app,
        By::Owner(Actor::User),
        Structure::Board(c::BoardUpdate {
            board_id: rs.id.clone(),
            version: rs_now.version + 1,
            structure_policy: Some("owner_confirms".into()),
            ..Default::default()
        }),
    )
    .expect("back to owner_confirms");
    let rs_now = db.board(&rs.id).expect("read").expect("board");
    let Outcome::Proposed { proposal_id, .. } =
        admin::structure(&d.app, as_lead(), add(rs_now.version, "Ops")).expect("proposed")
    else {
        panic!("expected a proposal");
    };
    admin::rule(&d.app, &Actor::User, &proposal_id, false, Some("not now")).expect("declined");
    assert!(!db
        .board_columns(&p.id)
        .expect("columns")
        .iter()
        .any(|col| col.name == "Ops"));
}

/// `board_get` is the boards a bot works on: an archived board's columns
/// and cards are left out, as `board_list` leaves out the board, and its
/// key is the default board's, the one `item_create` uses (0.19.0 K9).
#[tokio::test]
async fn board_get_leaves_out_an_archived_board_and_names_the_default_key() {
    let (pair, mut bots) = common::tasks::project_with_bots(&["Lead"]).await;
    let (d, lead) = (&pair.d, &mut bots[0]);
    let db = &d.app.db;
    let project = db.get_bot(&pair.ids[0]).unwrap().unwrap().project_id;
    db.ensure_board(&project, &db.daemon_id().unwrap(), Some("RB"))
        .unwrap();
    let main = db.default_board(&project).unwrap().expect("main");
    let recipes = db
        .create_board(&project, "Recipes", "RC", Preset::Simple)
        .unwrap();
    let owner = || By::Owner(Actor::User);
    let update = |board: &hermesd::kanban::model::Board| c::BoardUpdate {
        board_id: board.id.clone(),
        version: board.version,
        ..Default::default()
    };
    let default = Structure::Board(c::BoardUpdate {
        is_default: Some(true),
        ..update(&recipes)
    });
    admin::structure(&d.app, owner(), default).expect("default");
    let main = db.board(&main.id).unwrap().expect("main");
    let archive = Structure::Board(c::BoardUpdate {
        archived: Some(true),
        ..update(&main)
    });
    admin::structure(&d.app, owner(), archive).expect("archived");

    let got = lead.call("board_get", serde_json::json!({})).await;
    assert_eq!(got["key"], "RC", "{got}");
    let boards: Vec<&str> = got["columns"]
        .as_array()
        .expect("columns")
        .iter()
        .filter_map(|col| col["board_id"].as_str())
        .collect();
    assert!(!boards.is_empty());
    assert!(boards.iter().all(|b| *b == recipes.id), "{got}");
}
