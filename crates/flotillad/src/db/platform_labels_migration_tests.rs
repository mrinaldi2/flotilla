//! Platforms become labels (0.19.0 K8): a database as 0.18 left it, its
//! items' platforms set, migrates from 0.18's schema version through every
//! 0.19 step. Each platform becomes a `platform:` label, nothing is lost,
//! the release-gated rules that read platforms read the labels the same way,
//! and the migration runs again without changing anything.

use rusqlite::params;
use serde_json::{json, Value};

use super::board_tests::board;
use super::{Db, NewItem};
use crate::actor::Actor;
use crate::kanban::labels::LabelGroup;
use crate::kanban::model::{ItemType, LinkKind, Priority, ProjectRole, Role, Size, TemplateKind};
use crate::kanban::moves::{item_move, MoveRequest, Moved};

const OWNER: Actor<'static> = Actor::User;

/// What the migration may touch, and what it must keep, as one value.
fn state(db: &Db, p: &str) -> Value {
    let conn = db.lock();
    let rows = |sql: &str| -> Vec<Value> {
        let mut stmt = conn.prepare(sql).unwrap();
        let n = stmt.column_count();
        stmt.query_map(params![p], |r| {
            Ok(Value::Array(
                (0..n)
                    .map(|i| match r.get_ref(i).unwrap() {
                        rusqlite::types::ValueRef::Null => Value::Null,
                        rusqlite::types::ValueRef::Integer(v) => json!(v),
                        rusqlite::types::ValueRef::Real(v) => json!(v),
                        v => json!(v.as_str().unwrap()),
                    })
                    .collect(),
            ))
        })
        .unwrap()
        .map(Result::unwrap)
        .collect()
    };
    json!({
        "items": rows("SELECT id, title, labels, platforms, column_key, version FROM item
                       WHERE project_id = ?1 ORDER BY id"),
        "links": rows("SELECT l.item_id, l.kind, l.ref FROM item_link l JOIN item i
                       ON i.id = l.item_id WHERE i.project_id = ?1 ORDER BY l.id"),
        "events": rows("SELECT count(*) FROM item_event e JOIN item i ON i.id = e.item_id
                        WHERE i.project_id = ?1"),
        "templates": rows("SELECT name, version, body FROM template
                           WHERE project_id = ?1 ORDER BY name, version"),
        "settings": rows("SELECT required_machines FROM board_settings WHERE project_id = ?1"),
        "boards": rows("SELECT * FROM board WHERE project_id = ?1"),
        "roles": rows("SELECT role, bot_id, machine FROM project_role
                       WHERE project_id = ?1 ORDER BY role, bot_id"),
    })
}

fn item(db: &Db, p: &str, title: &str, item_type: ItemType, labels: &[&str]) -> String {
    let labels: Vec<String> = labels.iter().map(|l| (*l).to_string()).collect();
    let ac = ["it works".to_string()];
    db.create_item(
        &NewItem {
            project_id: p,
            board_id: None,
            item_type,
            title,
            description: "",
            size: Some(Size::M),
            priority: Priority::P2,
            labels: &labels,
            parent_id: None,
            acceptance_criteria: &ac,
        },
        &OWNER,
    )
    .unwrap()
    .id
}

/// The codes a lead's move to Ready is refused with, or none when it moves.
fn refine(db: &Db, id: &str, lead: &str, p: &str) -> Vec<String> {
    let current = db.get_item(id).unwrap().unwrap();
    let req = MoveRequest {
        id,
        to: "ready",
        expected_version: current.version,
        reason: None,
        override_reason: None,
    };
    let actor = Actor::Bot {
        id: lead,
        project_id: p,
    };
    match item_move(db, &req, &actor).unwrap() {
        Moved::Done(_) => Vec::new(),
        Moved::Refused(unmet) => unmet.into_iter().map(|u| u.code).collect(),
        other => panic!("{other:?}"),
    }
}

/// Back to what 0.18 stored: platforms on items and keys, the 0.18
/// templates, no `board` table rows nor the columns that came after.
fn as_018(db: &Db, platforms: &[(&str, &str, &str)]) {
    let conn = db.lock();
    for (id, platforms, labels) in platforms {
        conn.execute(
            "UPDATE item SET platforms = ?2, labels = ?3 WHERE id = ?1",
            params![id, platforms, labels],
        )
        .unwrap();
    }
    // 0.18 boards had feature templates, not task or design ones (0.21.0).
    conn.execute_batch(
        r#"DELETE FROM template WHERE name IN ('task', 'design');
           INSERT INTO template(project_id, kind, name, version, body, created_at)
               SELECT project_id, 'item_type', 'feature', 1,
                   '{"sections":["User/problem"],"ready":[],"flow":"full"}', created_at
               FROM board_settings;"#,
    )
    .unwrap();
    conn.execute_batch(
        r#"UPDATE template SET body = json_set(body, '$.ready', json(CASE name
               WHEN 'feature' THEN '["acceptance_criteria","platforms","size","spec_link_for_ui_or_daemon"]'
               WHEN 'bug' THEN '["steps_to_reproduce","expected_actual","acceptance_criteria","platforms"]'
               WHEN 'chore' THEN '["description","platforms"]'
               ELSE json_extract(body, '$.ready') END));
           UPDATE board_settings SET required_machines =
               '{"desktop":["mac","win-pc"],"ios":["iphone"]}';
           UPDATE item SET board_id = NULL;
           UPDATE board_column SET board_id = NULL, kind = NULL;
           UPDATE template SET board_id = NULL;
           DELETE FROM board;
           ALTER TABLE board DROP COLUMN label_groups;"#,
    )
    .unwrap();
}

fn migrate_from(db: &Db, marker: &str) {
    let from = bus::schema::MIGRATIONS
        .iter()
        .position(|m| m.contains(marker))
        .expect("the migration");
    db.lock()
        .execute(
            "UPDATE meta SET value = ?1 WHERE key = 'schema_version'",
            [from.to_string()],
        )
        .unwrap();
    db.migrate().unwrap();
}

#[test]
fn a_018_board_keeps_its_platforms_as_labels_and_its_rules() {
    let (db, p) = board();
    let lead = db
        .project_roles(&p)
        .unwrap()
        .into_iter()
        .find(|r| r.role == Role::Lead)
        .unwrap()
        .bot_id;
    let daemon = item(&db, &p, "Guards", ItemType::Task, &[]);
    let apps = item(&db, &p, "Both apps", ItemType::Task, &["urgent"]);
    let infra = item(&db, &p, "Backups", ItemType::Task, &[]);
    let none = item(&db, &p, "Somewhere", ItemType::Task, &[]);
    let twice = item(&db, &p, "Crash", ItemType::Bug, &["was:B1"]);
    db.board_tx(|t| crate::kanban::api::link(t, &apps, LinkKind::Branch, "feat", None, &OWNER))
        .unwrap();
    // 0.18 guessed this tester's role from its name and stored it.
    let tester = db
        .create_bot(&p, "Tester Win", "", "", "", "/tmp/t", "tester-win", None)
        .unwrap();
    db.set_project_role(&ProjectRole {
        project_id: p.clone(),
        role: Role::Tester,
        bot_id: tester.id,
        machine: Some("win-pc".into()),
    })
    .unwrap();
    as_018(
        &db,
        &[
            (&daemon, r#"["daemon"]"#, "[]"),
            (&apps, r#"["desktop","ios"]"#, r#"["urgent"]"#),
            (&infra, r#"["infra"]"#, r#"["platform:infra"]"#),
            (&none, "[]", "[]"),
            (&twice, r#"["ios","ios"]"#, r#"["was:B1"]"#),
        ],
    );
    let before = state(&db, &p);

    // From 0.18's schema version, through every 0.19 step.
    migrate_from(&db, "CREATE TABLE IF NOT EXISTS board (");
    let after = state(&db, &p);
    let labels = |id: &str| db.get_item(id).unwrap().unwrap().labels;
    assert_eq!(labels(&daemon), ["platform:daemon"]);
    assert_eq!(
        labels(&apps),
        ["urgent", "platform:desktop", "platform:ios"]
    );
    assert_eq!(labels(&infra), ["platform:infra"]);
    assert!(labels(&none).is_empty());
    assert_eq!(labels(&twice), ["was:B1", "platform:ios"]);
    assert!(after["items"]
        .as_array()
        .unwrap()
        .iter()
        .all(|i| i[3] == "[]"));
    for key in ["links", "events", "roles"] {
        assert_eq!(after[key], before[key], "{key}");
    }
    let ids = |s: &Value| -> Vec<Value> {
        s["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i[0].clone())
            .collect()
    };
    assert_eq!(ids(&after), ids(&before));

    let main = db.default_board(&p).unwrap().unwrap();
    assert_eq!(
        main.label_groups,
        [LabelGroup {
            name: "platform".into(),
            values: ["desktop", "ios", "daemon", "infra"]
                .map(String::from)
                .to_vec(),
        }]
    );
    let settings = db.board_settings(&p).unwrap().unwrap();
    assert_eq!(
        settings.required_machines.keys().collect::<Vec<_>>(),
        ["platform:desktop", "platform:ios"]
    );
    assert_eq!(
        settings.required_machines["platform:desktop"],
        ["mac", "win-pc"]
    );
    let ready = |name: &str| -> Value {
        db.templates(&p, TemplateKind::ItemType)
            .unwrap()
            .into_iter()
            .find(|t| t.name == name)
            .unwrap()
            .body["ready"]
            .clone()
    };
    // The feature and chore templates went with 0.21's card types; the bug
    // template keeps what this migration gave it.
    assert_eq!(
        ready("bug"),
        json!([
            "steps_to_reproduce",
            "expected_actual",
            "acceptance_criteria",
            "label:platform"
        ])
    );

    // No role is guessed any more, and none that was guessed is lost:
    // opening the board again leaves them as they were.
    let roles: Vec<String> = db
        .project_roles(&p)
        .unwrap()
        .iter()
        .map(|r| r.role.as_str().to_string())
        .collect();
    assert_eq!(roles, ["lead", "reviewer.arch", "tester"]);
    db.ensure_board(&p, "d-mac", None).unwrap();
    assert_eq!(state(&db, &p)["roles"], before["roles"]);

    // Again: nothing changes.
    migrate_from(&db, "ALTER TABLE board ADD COLUMN label_groups");
    assert_eq!(state(&db, &p), after);
    migrate_from(&db, "CREATE TABLE IF NOT EXISTS board (");
    assert_eq!(state(&db, &p), after);

    // The rules read the labels as they read platforms: an item for the
    // daemon or the apps needs its spec, an infra one doesn't, and one with
    // no platform is asked for it, in the same words.
    assert_eq!(refine(&db, &daemon, &lead, &p), ["dor.spec_link"]);
    assert_eq!(refine(&db, &infra, &lead, &p), Vec::<String>::new());
    assert_eq!(refine(&db, &none, &lead, &p), ["dor.label"]);
    let check = crate::kanban::moves::item_move_check(&db, &none, &OWNER).unwrap();
    let texts: Vec<&str> = check
        .iter()
        .flat_map(|(_, unmet)| unmet.iter())
        .filter(|u| u.code == "dor.label")
        .map(|u| u.text.as_str())
        .collect();
    assert!(texts.contains(&"Pick its platforms."), "{texts:?}");
}
