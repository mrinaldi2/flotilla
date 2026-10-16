//! A deleted project's half on a linked computer (0.19.0): the computer is
//! asked to trash it, at once or when its link comes back, answers, and a
//! computer that never answers (one from before 0.19) ends as not
//! confirming. Both daemons run here, over a real peer link.

use crate::common;

use common::peers::{computer, paired, paired_from, project, wait_until, Paired};
use common::{create_bot, setup_wait, WsClient};
use flotillad::trash::{bucket_dir, Bucket};
use serde_json::{json, Value};

/// The Mac's project `app` with a bot, linked with a mirror the PC made for
/// it. Returns the PC's project id and folder name.
async fn mirrored(p: &mut Paired) -> (String, String, String) {
    let mac_app = project(&mut p.mac_client, "app").await;
    create_bot(&mut p.mac_client, &mac_app, "lead").await;
    let reply = p
        .mac_client
        .request_within(
            setup_wait(),
            json!({ "type": "link_project", "project_id": mac_app, "peer_id": p.mac_peer_id }),
        )
        .await;
    assert_eq!(reply["type"], "project", "{reply}");
    let link = p
        .mac
        .app
        .db
        .project_link(&mac_app, &p.mac_peer_id)
        .unwrap()
        .unwrap();
    let win_app = link.remote_project_id;
    let dir = p
        .win
        .app
        .db
        .get_project(&win_app)
        .unwrap()
        .unwrap()
        .dir_name;
    (mac_app, win_app, dir)
}

async fn delete(c: &mut WsClient, id: &str) {
    let reply = c
        .request(json!({ "type": "delete_project", "project_id": id }))
        .await;
    assert_eq!(reply["type"], "ok", "{reply}");
}

fn order_state(p: &Paired, project_id: &str) -> Option<String> {
    ["pending", "confirmed", "unconfirmed"]
        .iter()
        .flat_map(|s| p.mac.app.db.trash_orders(s, None).unwrap())
        .find(|o| o.project_id == project_id)
        .map(|o| o.state)
}

fn trashed_on_the_pc(p: &Paired, dir: &str) -> bool {
    let bucket = bucket_dir(
        &p.win.app.cfg,
        Bucket {
            label: dir,
            group: "project-files",
        },
    );
    bucket.join(dir).is_dir()
}

#[tokio::test]
async fn the_mirror_on_a_linked_computer_is_trashed_with_the_project() {
    let mut p = paired().await;
    let (mac_app, win_app, dir) = mirrored(&mut p).await;
    let preview = p
        .mac_client
        .request(json!({ "type": "project_delete_preview", "project_id": mac_app }))
        .await;
    assert_eq!(preview["preview"]["peers"][0]["name"], "win", "{preview}");
    assert_eq!(preview["preview"]["peers"][0]["online"], true);

    delete(&mut p.mac_client, &mac_app).await;
    wait_until("the PC confirms", || {
        order_state(&p, &mac_app).as_deref() == Some("confirmed")
    })
    .await;
    let mirror = p.win.app.db.get_project(&win_app).unwrap().unwrap();
    assert!(
        mirror.deleted_at.is_some(),
        "the mirror is deleted there too"
    );
    assert!(
        trashed_on_the_pc(&p, &dir),
        "and its folder is in its trash"
    );
    assert!(!p.win.app.cfg.projects_dir().join(&dir).exists());
}

#[tokio::test]
async fn a_linked_project_with_its_own_team_is_kept() {
    let mut p = paired().await;
    let mac_app = project(&mut p.mac_client, "app").await;
    create_bot(&mut p.mac_client, &mac_app, "lead").await;
    let win_app = project(&mut p.win_client, "app").await;
    create_bot(&mut p.win_client, &win_app, "windev").await;
    let reply = p
        .mac_client
        .request_within(
            setup_wait(),
            json!({ "type": "link_project", "project_id": mac_app,
                    "peer_id": p.mac_peer_id, "remote_project_id": win_app }),
        )
        .await;
    assert_eq!(reply["type"], "project", "{reply}");

    delete(&mut p.mac_client, &mac_app).await;
    wait_until("the PC confirms", || {
        order_state(&p, &mac_app).as_deref() == Some("confirmed")
    })
    .await;
    let order = p
        .mac
        .app
        .db
        .trash_orders("confirmed", None)
        .unwrap()
        .remove(0);
    let outcome: Value = order.outcome.unwrap();
    assert_eq!(outcome["outcome"], "kept", "{outcome}");
    let kept = p.win.app.db.get_project(&win_app).unwrap().unwrap();
    assert!(kept.deleted_at.is_none(), "its own team still works there");
}

#[tokio::test]
async fn a_computer_offline_at_the_delete_is_asked_when_it_is_back() {
    let mut p = paired().await;
    let (mac_app, win_app, dir) = mirrored(&mut p).await;
    // The Mac drops the link; it redials no sooner than two seconds on.
    p.mac.app.peers.disconnect(&p.mac_peer_id);
    delete(&mut p.mac_client, &mac_app).await;
    let waiting = p.mac.app.db.trash_orders("pending", None).unwrap();
    assert_eq!(waiting.len(), 1, "kept for when it is back");
    assert_eq!(waiting[0].attempts, 0, "not sent while offline");
    assert!(p
        .win
        .app
        .db
        .get_project(&win_app)
        .unwrap()
        .unwrap()
        .deleted_at
        .is_none());

    wait_until("the PC confirms once it is back", || {
        order_state(&p, &mac_app).as_deref() == Some("confirmed")
    })
    .await;
    assert!(p
        .win
        .app
        .db
        .get_project(&win_app)
        .unwrap()
        .unwrap()
        .deleted_at
        .is_some());
    assert!(trashed_on_the_pc(&p, &dir));
}

#[tokio::test]
async fn a_computer_that_never_answers_ends_as_not_confirming() {
    let mac = computer(|cfg| {
        cfg.trash.peer_resend_minutes = 0;
        cfg.trash.peer_attempts = 2;
    })
    .await;
    // As a computer from before 0.19: the ask is an event it doesn't know.
    let win = computer(|cfg| cfg.ignore_trash_asks_for_tests(true)).await;
    let mut p = paired_from(mac, win).await;
    let (mac_app, win_app, _) = mirrored(&mut p).await;

    delete(&mut p.mac_client, &mac_app).await;
    let mut ended = 0;
    for _ in 0..5 {
        ended += flotillad::project_trash::peer::sweep(&p.mac.app);
    }
    assert_eq!(ended, 1);
    assert_eq!(order_state(&p, &mac_app).as_deref(), Some("unconfirmed"));
    let notice = p
        .mac_client
        .wait_for(|m| m["type"] == "notify" && m["title"] == "Linked computer didn't confirm")
        .await;
    assert!(
        notice["body"]
            .as_str()
            .unwrap()
            .starts_with("1 linked computer"),
        "{notice}"
    );
    let preview = p
        .mac_client
        .request(json!({ "type": "deleted_projects_preview" }))
        .await;
    assert_eq!(
        preview["preview"]["linked"][0]["state"], "unconfirmed",
        "{preview}"
    );
    let there = p.win.app.db.get_project(&win_app).unwrap().unwrap();
    assert!(there.deleted_at.is_none(), "it ignored the ask");
}
