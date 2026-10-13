//! Many boards per project (0.19.0 K2): a new board and a migrated one both
//! end up as the project's default board, with every column, item, link and
//! event where it was.

use rusqlite::params;

use super::board_tests::{board, new_item};
use super::Db;
use crate::actor::Actor;
use crate::kanban::model::{ColumnKind, LinkKind, StructurePolicy};

const OWNER: Actor<'static> = Actor::User;

fn counts(db: &Db, p: &str) -> (u32, u32, u32, u32) {
    let conn = db.lock();
    let count = |sql: &str| -> u32 { conn.query_row(sql, params![p], |r| r.get(0)).unwrap() };
    (
        count("SELECT count(*) FROM item WHERE project_id = ?1"),
        count("SELECT count(*) FROM board_column WHERE project_id = ?1"),
        count(
            "SELECT count(*) FROM item_link JOIN item ON item.id = item_link.item_id
             WHERE item.project_id = ?1",
        ),
        count(
            "SELECT count(*) FROM item_event JOIN item ON item.id = item_event.item_id
             WHERE item.project_id = ?1",
        ),
    )
}

#[test]
fn a_new_board_is_the_projects_default_board() {
    let (db, p) = board();
    let boards = db.boards(&p).unwrap();
    assert_eq!(boards.len(), 1);
    let main = &boards[0];
    assert!(main.is_default);
    assert_eq!((main.name.as_str(), main.key.as_str()), ("Main", "H"));
    assert_eq!(main.structure_policy, StructurePolicy::OwnerConfirms);
    let columns = db.board_columns(&p).unwrap();
    assert!(columns.iter().all(|c| c.board_id == main.id));
    let kind = |key: &str| columns.iter().find(|c| c.key == key).unwrap().kind;
    assert_eq!(kind("inbox"), ColumnKind::Open);
    assert_eq!(kind("review"), ColumnKind::Active);
    assert_eq!(kind("done"), ColumnKind::Done);
    assert_eq!(kind("cancelled"), ColumnKind::Cancelled);
    let item = db.create_item(&new_item(&p, "Board"), &OWNER).unwrap();
    assert_eq!(item.board_id, main.id);
    assert!(item.id.starts_with("H-"));
}

/// A board from before 0.19 (no `board` row, no `board_id` or `kind`)
/// becomes the default board, keeping its key, and the migration runs again
/// without changing anything.
#[test]
fn an_older_board_migrates_into_the_default_board_and_runs_again() {
    let (db, p) = board();
    for title in ["One", "Two", "Three"] {
        let item = db.create_item(&new_item(&p, title), &OWNER).unwrap();
        db.board_tx(|t| {
            crate::kanban::api::link(t, &item.id, LinkKind::Branch, "feat", None, &OWNER)
        })
        .unwrap();
    }
    let before = counts(&db, &p);
    let ids: Vec<String> = {
        let conn = db.lock();
        let mut stmt = conn
            .prepare("SELECT id FROM item WHERE project_id = ?1 ORDER BY id")
            .unwrap();
        stmt.query_map([&p], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    };
    // Back to what 0.18 stored.
    db.lock()
        .execute_batch(
            "UPDATE item SET board_id = NULL;
             UPDATE board_column SET board_id = NULL, kind = NULL;
             DELETE FROM board;",
        )
        .unwrap();
    let ours = bus::schema::MIGRATIONS
        .iter()
        .position(|m| m.contains("CREATE TABLE IF NOT EXISTS board ("))
        .expect("the boards migration");
    for _ in 0..2 {
        db.lock()
            .execute(
                "UPDATE meta SET value = ?1 WHERE key = 'schema_version'",
                [ours.to_string()],
            )
            .unwrap();
        db.migrate().unwrap();
    }
    assert_eq!(counts(&db, &p), before);
    let main = db.default_board(&p).unwrap().expect("default board");
    assert_eq!(
        (main.id.as_str(), main.key.as_str()),
        (format!("{p}/main").as_str(), "H")
    );
    assert!(db
        .board_columns(&p)
        .unwrap()
        .iter()
        .all(|c| c.board_id == main.id));
    for id in ids {
        assert_eq!(db.get_item(&id).unwrap().unwrap().board_id, main.id, "{id}");
    }
}

/// Migration 67 rebuilds `item_link` for the `thread` kind: from a 0.18
/// table (no `thread` in its CHECK) every link keeps its id, the rebuild
/// runs again unchanged, and new links number on.
#[test]
fn the_link_rebuild_keeps_every_link_and_runs_again() {
    let (db, p) = board();
    let item = db.create_item(&new_item(&p, "Linked"), &OWNER).unwrap();
    for target in ["feat-a", "feat-b"] {
        db.board_tx(|t| crate::kanban::api::link(t, &item.id, LinkKind::Branch, target, None, &OWNER))
            .unwrap();
    }
    let links = |db: &Db| -> Vec<(i64, String, String)> {
        let conn = db.lock();
        let mut stmt = conn
            .prepare("SELECT id, kind, ref FROM item_link ORDER BY id")
            .unwrap();
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    };
    // Back to the 0.18 table, whose CHECK has no `thread`.
    db.lock()
        .execute_batch(
            "CREATE TABLE item_link_old AS SELECT * FROM item_link;
             DROP TABLE item_link;
             CREATE TABLE item_link (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 item_id TEXT NOT NULL REFERENCES item(id),
                 kind TEXT NOT NULL CHECK(kind IN ('task', 'decision', 'artifact', 'branch',
                     'pr', 'meeting', 'item:blocks', 'item:relates', 'item:duplicates')),
                 ref TEXT NOT NULL, label TEXT, created_by TEXT NOT NULL, at TEXT NOT NULL,
                 UNIQUE (item_id, kind, ref));
             INSERT INTO item_link SELECT * FROM item_link_old;
             DROP TABLE item_link_old;",
        )
        .unwrap();
    let before = links(&db);
    assert_eq!(before.len(), 2);
    let ours = bus::schema::MIGRATIONS
        .iter()
        .position(|m| m.contains("CREATE TABLE item_link_next"))
        .expect("the drafts migration");
    for _ in 0..2 {
        db.lock()
            .execute(
                "UPDATE meta SET value = ?1 WHERE key = 'schema_version'",
                [ours.to_string()],
            )
            .unwrap();
        db.migrate().unwrap();
        assert_eq!(links(&db), before);
    }
    db.board_tx(|t| t.add_item_link(&item.id, LinkKind::Thread, "bot-1", Some("Lead"), &OWNER))
        .unwrap();
    let after = links(&db);
    assert_eq!(after.len(), 3);
    assert!(after[2].0 > before[1].0, "{after:?}");
    let unique: i64 = db
        .lock()
        .query_row(
            "SELECT count(*) FROM pragma_index_list('item_link') WHERE \"unique\" = 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(unique, 1, "the (item, kind, ref) key is kept");
}
