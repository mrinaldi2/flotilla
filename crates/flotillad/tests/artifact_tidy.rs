//! The owner's tidy-up of loose artifacts (0.22.0, `artifacts_v2`):
//! `artifact_keep` marks a file kept, `artifact_trash` moves one no card
//! uses to the trash. Nothing else is ever removed.

use serde_json::{json, Value};

use crate::artifact_groups::{attach_doc, list, rels, setup, write, Setup};
use crate::common::board::new_item;
use flotillad::kanban::model::Priority;

async fn ask(s: &mut Setup, req: Value) -> Value {
    s.c.request(req).await
}

fn kept(files: &[Value], rel: &str) -> Value {
    files.iter().find(|a| a["rel"] == rel).expect(rel)["kept"].clone()
}

#[tokio::test]
async fn the_owner_keeps_a_loose_artifact_and_takes_the_mark_off() {
    let mut s = setup().await;
    write(&s.artifacts, "notes.md");
    write(&s.artifacts, "items/KEY-1/screen.md");
    assert_eq!(kept(&list(&mut s, None).await, "notes.md"), false);

    let project_id = s.project_id.clone();
    let reply = ask(
        &mut s,
        json!({"type": "artifact_keep", "project_id": project_id, "path": "notes.md", "keep": true}),
    )
    .await;
    assert_eq!(reply["type"], "artifact_kept", "{reply}");
    assert_eq!(reply["project_id"], json!(project_id));
    assert_eq!(reply["artifact"]["rel"], "notes.md");
    assert_eq!(reply["artifact"]["kept"], true);
    assert_eq!(reply["artifact"]["group"], "shared");
    assert_eq!(kept(&list(&mut s, Some("shared")).await, "notes.md"), true);

    let reply = ask(
        &mut s,
        json!({"type": "artifact_keep", "project_id": project_id, "path": "notes.md", "keep": false}),
    )
    .await;
    assert_eq!(reply["artifact"]["kept"], false, "{reply}");
    assert_eq!(kept(&list(&mut s, None).await, "notes.md"), false);

    // Only a listed file, named by its path in the artifacts folder.
    for (path, code) in [
        ("items/KEY-1/screen.md", "invalid_request"),
        (".cache/x.md", "invalid_request"),
        ("../secret.md", "invalid_request"),
        ("missing.md", "not_found"),
    ] {
        let refused = ask(
            &mut s,
            json!({"type": "artifact_keep", "project_id": project_id, "path": path, "keep": true}),
        )
        .await;
        assert_eq!(refused["type"], "error", "{path}: {refused}");
        assert_eq!(refused["code"], code, "{path}: {refused}");
    }
}

#[tokio::test]
async fn a_loose_artifact_goes_to_the_trash_and_a_used_one_stays() {
    let mut s = setup().await;
    for rel in ["loose.md", "spec.md", "items/KEY-1/screen.md"] {
        write(&s.artifacts, rel);
    }
    let (item, _) = new_item(&s.d.app.db, &s.project_id, "Login", Priority::P2);
    attach_doc(&s, &item, "spec.md");
    let project_id = s.project_id.clone();
    ask(
        &mut s,
        json!({"type": "artifact_keep", "project_id": project_id, "path": "loose.md", "keep": true}),
    )
    .await;

    let trashed = ask(
        &mut s,
        json!({"type": "artifact_trash", "project_id": project_id, "path": "loose.md"}),
    )
    .await;
    assert_eq!(trashed["type"], "artifact_trashed", "{trashed}");
    assert_eq!(trashed["path"], "loose.md");
    assert_eq!(trashed["project_id"], json!(project_id));
    assert!(!s.artifacts.join("loose.md").exists());
    assert_eq!(rels(&list(&mut s, None).await), ["spec.md"]);
    // Moved, not erased: test builds use the trash folder in their home.
    let project = s.d.app.db.get_project(&project_id).unwrap().unwrap();
    let bucket = flotillad::trash::Bucket {
        label: &project.dir_name,
        group: "artifacts",
    };
    let bin = flotillad::trash::bucket_dir(&s.d.app.cfg, bucket);
    assert!(bin.exists(), "{}", bin.display());
    assert!(s.d.app.db.kept_artifacts(&project_id).unwrap().is_empty());

    // A card still uses it: refused, and the file stays.
    let refused = ask(
        &mut s,
        json!({"type": "artifact_trash", "project_id": project_id, "path": "spec.md"}),
    )
    .await;
    assert_eq!(refused["code"], "conflict", "{refused}");
    assert!(
        refused["message"].as_str().unwrap().contains(&item),
        "{refused}"
    );
    assert!(s.artifacts.join("spec.md").is_file());

    // The storage folders and anything outside are never trashed.
    for (path, code) in [
        ("items/KEY-1/screen.md", "invalid_request"),
        ("/etc/hosts", "invalid_request"),
        ("gone.md", "not_found"),
    ] {
        let refused = ask(
            &mut s,
            json!({"type": "artifact_trash", "project_id": project_id, "path": path}),
        )
        .await;
        assert_eq!(refused["code"], code, "{path}: {refused}");
    }
    assert!(s.artifacts.join("items/KEY-1/screen.md").is_file());
}
