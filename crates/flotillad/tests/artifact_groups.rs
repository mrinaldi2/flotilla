//! Artifacts by kind (0.22.0, `artifacts_v2`): `list_artifacts` groups, the
//! cards that use each file, the storage folders left out, and a card's
//! linked artifacts among its documents.

use std::path::{Path, PathBuf};

use crate::common;

use bus::contract::board::{self as c, board_request::Request, board_response::Response};
use common::board::*;
use common::{raw_hello, spawn_daemon, TestDaemon, WsClient};
use flotillad::actor::Actor;
use flotillad::db::NewAttachment;
use flotillad::kanban::model::{AttachmentKind, LinkKind, Priority};
use serde_json::{json, Value};

struct Setup {
    d: TestDaemon,
    c: WsClient,
    project_id: String,
    artifacts: PathBuf,
}

async fn setup() -> Setup {
    let d = spawn_daemon().await;
    let mut c = WsClient::connect(&d).await;
    let created = c
        .request(json!({"type": "create_project", "name": "app"}))
        .await;
    let project_id = created["project"]["id"].as_str().expect("pid").to_string();
    let db = &d.app.db;
    db.ensure_board(&project_id, &db.daemon_id().unwrap(), Some("A"))
        .unwrap();
    let project = db.get_project(&project_id).unwrap().expect("project");
    let artifacts = flotillad::paths::artifacts_dir(&d.app.cfg, &project.dir_name);
    Setup {
        d,
        c,
        project_id,
        artifacts,
    }
}

fn write(artifacts: &Path, rel: &str) {
    let path = rel
        .split('/')
        .fold(artifacts.to_path_buf(), |p, s| p.join(s));
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, format!("# {rel}\n")).unwrap();
}

/// Attaches the artifact at `rel` to `item` as a doc, as `item_attach` does.
fn attach_doc(s: &Setup, item: &str, rel: &str) -> String {
    let new = NewAttachment {
        item_id: item,
        kind: AttachmentKind::Doc,
        name: rel.rsplit('/').next().unwrap(),
        mime: "",
        bytes: 0,
        sha256: "",
        path: rel,
        url: "",
        caption: "",
    };
    let db = &s.d.app.db;
    db.board_tx(|t| t.add_attachment(&new, &Actor::User))
        .unwrap()
        .id
}

fn link(s: &Setup, item: &str, target: &str, label: Option<&str>) {
    s.d.app
        .db
        .add_item_link(item, LinkKind::Artifact, target, label, &Actor::User)
        .unwrap();
}

async fn list(s: &mut Setup, group: Option<&str>) -> Vec<Value> {
    let mut req = json!({"type": "list_artifacts", "project_id": s.project_id});
    if let Some(group) = group {
        req["group"] = json!(group);
    }
    let reply = s.c.request(req).await;
    assert_eq!(reply["type"], "artifacts", "{reply}");
    let mut files = reply["artifacts"].as_array().expect("artifacts").clone();
    files.sort_by_key(|a| a["rel"].as_str().unwrap_or_default().to_string());
    files
}

fn rels(files: &[Value]) -> Vec<&str> {
    files.iter().map(|a| a["rel"].as_str().unwrap()).collect()
}

fn used_by<'a>(files: &'a [Value], rel: &str) -> Vec<&'a str> {
    let file = files.iter().find(|a| a["rel"] == rel).expect(rel);
    file["used_by"]
        .as_array()
        .expect("used_by")
        .iter()
        .map(|id| id.as_str().unwrap())
        .collect()
}

#[tokio::test]
async fn artifacts_are_listed_by_group_without_the_storage_folders() {
    let mut s = setup().await;
    for rel in [
        "notes.md",
        "runs/1/frame.md",
        "uploads/shot.md",
        "checks/ck-1/build.log",
        "items/KEY-1/screen.md",
        ".cache/peer/file.md",
    ] {
        write(&s.artifacts, rel);
    }

    // Without a group: every kind, and never the storage folders.
    let all = list(&mut s, None).await;
    assert_eq!(
        rels(&all),
        [
            "checks/ck-1/build.log",
            "notes.md",
            "runs/1/frame.md",
            "uploads/shot.md"
        ]
    );
    let group = |rel: &str| all.iter().find(|a| a["rel"] == rel).unwrap()["group"].clone();
    assert_eq!(group("notes.md"), "shared");
    assert_eq!(group("runs/1/frame.md"), "shared");
    assert_eq!(group("uploads/shot.md"), "uploads");
    assert_eq!(group("checks/ck-1/build.log"), "checks");

    let shared = list(&mut s, Some("shared")).await;
    assert_eq!(rels(&shared), ["notes.md", "runs/1/frame.md"]);
    let uploads = list(&mut s, Some("uploads")).await;
    assert_eq!(rels(&uploads), ["uploads/shot.md"]);
    let checks = list(&mut s, Some("checks")).await;
    assert_eq!(rels(&checks), ["checks/ck-1/build.log"]);

    let refused =
        s.c.request(json!({
            "type": "list_artifacts", "project_id": s.project_id, "group": "items"
        }))
        .await;
    assert_eq!(refused["type"], "error", "{refused}");
    assert_eq!(refused["code"], "invalid_request", "{refused}");

    let hello = raw_hello(&s.d, s.d.app.secrets.client_token()).await;
    let capabilities = hello["capabilities"].as_array().expect("capabilities");
    assert!(capabilities.contains(&json!("artifacts_v2")), "{hello}");
}

#[tokio::test]
async fn each_artifact_lists_the_live_cards_that_attach_or_link_it() {
    let mut s = setup().await;
    for rel in ["spec.md", "data/set.csv", "loose.md", "uploads/brief.md"] {
        write(&s.artifacts, rel);
    }
    let db = &s.d.app.db;
    let (first, _) = new_item(db, &s.project_id, "Login", Priority::P2);
    let (second, _) = new_item(db, &s.project_id, "Signup", Priority::P2);

    attach_doc(&s, &first, "spec.md");
    // The same file linked, by its absolute path: one use per card.
    let absolute = s.artifacts.join("spec.md").display().to_string();
    link(&s, &first, &absolute, None);
    link(&s, &second, "data/set.csv", Some("Data set"));
    link(&s, &second, "spec.md", None);
    attach_doc(&s, &second, "uploads/brief.md");
    // A detached file no longer counts.
    let gone = attach_doc(&s, &first, "loose.md");
    db.board_tx(|t| t.detach(&first, &gone, &Actor::User))
        .unwrap();

    let all = list(&mut s, None).await;
    assert_eq!(used_by(&all, "spec.md"), [first.as_str(), second.as_str()]);
    assert_eq!(used_by(&all, "data/set.csv"), [second.as_str()]);
    assert_eq!(used_by(&all, "uploads/brief.md"), [second.as_str()]);
    assert!(used_by(&all, "loose.md").is_empty(), "{all:?}");

    // A group's page carries them too.
    let uploads = list(&mut s, Some("uploads")).await;
    assert_eq!(used_by(&uploads, "uploads/brief.md"), [second.as_str()]);
}

#[tokio::test]
async fn a_cards_linked_artifacts_are_among_its_documents() {
    let mut s = setup().await;
    for rel in ["research.md", "report.html", "spec.md"] {
        write(&s.artifacts, rel);
    }
    let db = &s.d.app.db;
    let (item, _) = new_item(db, &s.project_id, "Search", Priority::P2);
    attach_doc(&s, &item, "spec.md");
    link(&s, &item, "research.md", Some("Research"));
    let absolute = s.artifacts.join("report.html").display().to_string();
    link(&s, &item, &absolute, None);
    // Already attached: shown once.
    link(&s, &item, "spec.md", None);
    // Not a file in the artifacts folder: a link only.
    link(&s, &item, "missing.md", None);
    link(&s, &item, "/elsewhere/notes.md", None);

    let detail =
        match response(call(&mut s.c, Request::ItemGet(c::ItemGet { id: item.clone() })).await) {
            Response::Item(detail) => detail,
            other => panic!("expected the item, got {other:?}"),
        };
    assert_eq!(detail.links.len(), 5, "links stay as they are");
    let docs: Vec<(&str, &str)> = detail
        .attachments
        .iter()
        .map(|a| (a.name.as_str(), a.id.as_str()))
        .collect();
    assert_eq!(docs.len(), 3, "{docs:?}");
    assert_eq!(docs[0].0, "spec.md");
    assert_eq!(docs[1], ("Research", "link:research.md"));
    assert_eq!(docs[2].0, "report.html");
    assert_eq!(docs[2].1, format!("link:{absolute}"));
    for doc in &detail.attachments[1..] {
        assert_eq!(doc.kind, "doc");
        assert!(!doc.removed);
        assert!(doc.url.is_empty());
        assert!(Path::new(&doc.path).is_file(), "{}", doc.path);
        assert!(doc.bytes > 0);
    }
    assert_eq!(
        Path::new(&detail.attachments[1].path),
        s.artifacts.join("research.md")
    );
}
