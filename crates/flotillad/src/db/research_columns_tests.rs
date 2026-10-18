//! The Research column reaching boards made before 0.21.0.

use super::board_tests::board;
use super::Db;
use crate::kanban::flow::EntryRule;
use crate::kanban::model::{BoardColumn, ColumnCategory};
use crate::kanban::presets::Preset;

/// Back to a board from before 0.21: no Research column, nothing added yet.
fn as_020(db: &Db) {
    let conn = db.lock();
    let doing_rules = serde_json::to_string(&[EntryRule::AssigneeSet]).unwrap();
    conn.execute_batch(&format!(
        "DELETE FROM board_column WHERE category = 'research';
         UPDATE board_column SET rules = '{doing_rules}' WHERE key = 'sr-doing';
         UPDATE board SET automations = json_set(automations, '$.\"work.started\"', 'sr-doing')
             WHERE key = 'SR';
         DELETE FROM meta WHERE key = 'research_columns_added';"
    ))
    .unwrap();
}

fn on(db: &Db, p: &str, board_id: &str) -> Vec<BoardColumn> {
    let mut cols: Vec<BoardColumn> = db
        .board_columns(p)
        .unwrap()
        .into_iter()
        .filter(|c| c.board_id == board_id)
        .collect();
    cols.sort_by_key(|c| c.ord);
    cols
}

#[test]
fn boards_from_before_get_research_before_doing_once() {
    let (db, p) = board();
    let gated = db.boards(&p).unwrap().remove(0);
    let simple = db
        .create_board(&p, "Second", "SR", Preset::SimpleReview)
        .unwrap();
    as_020(&db);
    db.backfill_research_columns().unwrap();

    let keys: Vec<String> = on(&db, &p, &gated.id).into_iter().map(|c| c.key).collect();
    assert_eq!(&keys[..4], ["inbox", "ready", "research", "doing"]);

    let cols = on(&db, &p, &simple.id);
    let keys: Vec<&str> = cols.iter().map(|c| c.key.as_str()).collect();
    assert_eq!(&keys[..3], ["todo", "sr-research", "sr-doing"]);
    assert_eq!(cols[1].category, ColumnCategory::Research);
    assert_eq!(cols[1].policy.movers, cols[2].policy.movers);
    assert!(cols[2].policy.rules.contains(&EntryRule::ResearchRecorded));
    let board = db
        .boards(&p)
        .unwrap()
        .into_iter()
        .find(|b| b.key == "SR")
        .unwrap();
    assert_eq!(board.automations["work.started"], "sr-research");

    // The owner removes it: it stays removed.
    db.lock()
        .execute("DELETE FROM board_column WHERE key = 'sr-research'", [])
        .unwrap();
    db.backfill_research_columns().unwrap();
    assert!(on(&db, &p, &simple.id)
        .iter()
        .all(|c| c.key != "sr-research"));
}
