//! The 0.21 card types migration on a board from before it: epic and feature
//! cards go with what hangs off them, chores become tasks, the rest stays,
//! and running it again changes nothing.

use rusqlite::params;

use super::board_tests::{board, new_item, OWNER};
use super::card_types::dry_run;
use super::Db;
use crate::kanban::model::{ItemType, TemplateKind};

/// The board as 0.20 left it: `item.type` taking the old types too, the
/// given cards retyped, and the schema rewound to just before the migration.
fn as_020(db: &Db, types: &[(&str, &str)]) -> usize {
    let conn = db.lock();
    let sql: String = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'item'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let body = &sql[sql.find('(').unwrap()..];
    let old = body.replace(
        "CHECK(type IN ('task', 'design', 'bug', 'spike'))",
        "CHECK(type IN ('epic', 'feature', 'bug', 'spike', 'chore'))",
    );
    assert_ne!(old, body, "the type CHECK is where the migration put it");
    conn.execute_batch(&format!(
        "PRAGMA foreign_keys = OFF;
         CREATE TABLE item_old {old};
         UPDATE item SET type = 'bug' WHERE type = 'task';
         INSERT INTO item_old SELECT * FROM item;
         DROP TABLE item;
         ALTER TABLE item_old RENAME TO item;
         PRAGMA foreign_keys = ON;"
    ))
    .unwrap();
    for (id, item_type) in types {
        conn.execute(
            "UPDATE item SET type = ?2 WHERE id = ?1",
            params![id, item_type],
        )
        .unwrap();
    }
    let before = bus::schema::MIGRATIONS
        .iter()
        .position(|sql| sql.contains("CREATE TABLE IF NOT EXISTS item_purged"))
        .expect("the card types migration");
    conn.execute(
        "UPDATE meta SET value = ?1 WHERE key = 'schema_version'",
        [before.to_string()],
    )
    .unwrap();
    before
}

fn count(db: &Db, sql: &str, id: &str) -> i64 {
    db.lock().query_row(sql, [id], |r| r.get(0)).unwrap()
}

#[test]
fn epics_and_features_go_chores_become_tasks_and_the_rest_stays() {
    let (db, p) = board();
    let make = |title: &str| db.create_item(&new_item(&p, title), &OWNER).unwrap().id;
    let epic = make("Umbrella");
    let feature = make("Board glance");
    let chore = make("Bump deps");
    let child = make("A child");
    let bug = make("Crash on open");
    {
        let conn = db.lock();
        conn.execute(
            "UPDATE item SET parent_id = ?2 WHERE id = ?1",
            params![child, epic],
        )
        .unwrap();
        for (on, kind, target) in [
            (&feature, "artifact", "spec.md"),
            (&bug, "item:relates", feature.as_str()),
        ] {
            conn.execute(
                "INSERT INTO item_link(item_id, kind, ref, created_by, at)
                 VALUES (?1, ?2, ?3, 'user', '2026-10-17T00:00:00Z')",
                params![on, kind, target],
            )
            .unwrap();
        }
        conn.execute(
            "INSERT INTO item_comment(id, item_id, author, body, at)
             VALUES ('c1', ?1, 'user', 'why', '2026-10-17T00:00:00Z')",
            [&feature],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO item_attachment(id, item_id, kind, name, added_by, at)
             VALUES ('a1', ?1, 'file', 'spec.md', 'user', '2026-10-17T00:00:00Z')",
            [&feature],
        )
        .unwrap();
    }
    as_020(
        &db,
        &[(&epic, "epic"), (&feature, "feature"), (&chore, "chore")],
    );
    db.migrate().unwrap();

    let gone = "SELECT count(*) FROM item WHERE id = ?1";
    assert_eq!(count(&db, gone, &epic), 0);
    assert_eq!(count(&db, gone, &feature), 0);
    assert_eq!(count(&db, gone, &bug), 1);
    for table in ["item_link", "item_comment", "item_attachment", "item_event"] {
        let sql = format!("SELECT count(*) FROM {table} WHERE item_id = ?1");
        assert_eq!(count(&db, &sql, &feature), 0, "{table}");
    }
    let relates = "SELECT count(*) FROM item_link WHERE item_id = ?1 AND kind = 'item:relates'";
    assert_eq!(
        count(&db, relates, &bug),
        0,
        "a link to a removed card goes too"
    );
    let parent: Option<String> = db
        .lock()
        .query_row("SELECT parent_id FROM item WHERE id = ?1", [&child], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(parent, None, "the child stays without its epic");
    assert_eq!(
        db.get_item(&chore).unwrap().unwrap().item_type,
        ItemType::Task
    );

    let pending = db.retired_cards_pending().unwrap();
    let ids: Vec<&str> = pending.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids.len(), 2);
    assert!(ids.contains(&epic.as_str()) && ids.contains(&feature.as_str()));
    db.retired_card_handled(&epic).unwrap();
    assert_eq!(db.retired_cards_pending().unwrap().len(), 1);

    // References stay whole, the search index lost the removed cards, and a
    // design link and a research column now fit.
    let broken: i64 = db
        .lock()
        .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(broken, 0);
    assert_eq!(
        count(
            &db,
            "SELECT count(*) FROM item_fts WHERE item_id = ?1",
            &feature
        ),
        0
    );
    db.lock()
        .execute(
            "INSERT INTO item_link(item_id, kind, ref, created_by, at)
             VALUES (?1, 'design', 'X-9', 'user', '2026-10-17T00:00:00Z')",
            [&bug],
        )
        .unwrap();
    let created = db.create_item(&new_item(&p, "Searchable"), &OWNER).unwrap();
    assert_eq!(
        count(
            &db,
            "SELECT count(*) FROM item_fts WHERE item_id = ?1",
            &created.id
        ),
        1,
        "the search triggers are back"
    );

    // Running it again (a rewound version) removes nothing more.
    let kept: i64 = db
        .lock()
        .query_row("SELECT count(*) FROM item", [], |r| r.get(0))
        .unwrap();
    let before = bus::schema::MIGRATIONS.len() - 1;
    db.lock()
        .execute(
            "UPDATE meta SET value = ?1 WHERE key = 'schema_version'",
            [before.to_string()],
        )
        .unwrap();
    db.migrate().unwrap();
    let after: i64 = db
        .lock()
        .query_row("SELECT count(*) FROM item", [], |r| r.get(0))
        .unwrap();
    assert_eq!(after, kept);
}

#[test]
fn new_templates_reach_boards_that_had_the_old_ones() {
    let (db, p) = board();
    db.lock()
        .execute(
            "DELETE FROM template WHERE project_id = ?1 AND name IN ('task', 'design')",
            [&p],
        )
        .unwrap();
    db.backfill_card_type_templates().unwrap();
    db.backfill_card_type_templates().unwrap();
    let names: Vec<String> = db
        .templates(&p, TemplateKind::ItemType)
        .unwrap()
        .into_iter()
        .map(|t| t.name)
        .collect();
    assert_eq!(names, ["bug", "design", "spike", "task"]);
    let versions: i64 = db
        .lock()
        .query_row(
            "SELECT count(*) FROM template WHERE project_id = ?1 AND name = 'task'",
            [&p],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(versions, 1, "seeded once");
}

#[test]
fn the_dry_run_lists_what_an_upgrade_removes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("flotilla.db");
    let db = Db::open(&path).unwrap();
    let (cards, chores) = dry_run(&path).unwrap();
    assert!(cards.is_empty());
    assert_eq!(chores, 0);
    drop(db);
}

/// The migration on a copy of a real database (`FLOTILLA_DB_COPY=<path>`):
/// it opens, every reference stays whole, and only epics and features go.
/// Never run against a live database: opening migrates it.
#[test]
#[ignore = "needs FLOTILLA_DB_COPY"]
fn a_real_database_copy_migrates() {
    let path = std::env::var("FLOTILLA_DB_COPY").expect("FLOTILLA_DB_COPY");
    let path = std::path::Path::new(&path);
    let (removed, chores) = dry_run(path).unwrap();
    let before: i64 = rusqlite::Connection::open(path)
        .unwrap()
        .query_row("SELECT count(*) FROM item", [], |r| r.get(0))
        .unwrap();
    let db = Db::open(path).unwrap();
    let conn = db.lock();
    let after: i64 = conn
        .query_row("SELECT count(*) FROM item", [], |r| r.get(0))
        .unwrap();
    let broken: i64 = conn
        .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| {
            r.get(0)
        })
        .unwrap();
    let types: Vec<(String, i64)> = conn
        .prepare("SELECT type, count(*) FROM item GROUP BY type ORDER BY type")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    println!(
        "removed {} of {before}, {chores} chores, now {after}: {types:?}",
        removed.len()
    );
    assert_eq!(after, before - removed.len() as i64);
    assert_eq!(broken, 0);
}
