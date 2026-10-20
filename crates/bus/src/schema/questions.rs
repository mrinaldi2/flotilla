//! Questions for the owner (0.22.0, questions.proto). A question is a
//! decision of kind `question` (migration 012); this table holds what only a
//! question has: the plan node it is about, whether it blocks that node's
//! research and gate, the card comment that announced it, where its answer
//! was written back, the owner's reason for dismissing it, and a version the
//! apps answer against. Its card is `decision.item_id` (migration 021),
//! copied here so a card's and a node's questions are one indexed read.
//!
//! Any change to the decision bumps the version, so an answer made through
//! the decision registry still conflicts with a stale one.

pub(super) const MIGRATION_QUESTIONS: &str = r#"
CREATE TABLE IF NOT EXISTS question (
    decision_id      TEXT PRIMARY KEY REFERENCES decision(id) ON DELETE CASCADE,
    project_id       TEXT NOT NULL,
    item_id          TEXT,
    plan_node_id     TEXT,
    blocking         INTEGER NOT NULL DEFAULT 0,
    comment_id       TEXT,
    written_to       TEXT,
    dismissed_reason TEXT,
    version          INTEGER NOT NULL DEFAULT 1
);
CREATE INDEX IF NOT EXISTS question_project ON question(project_id);
CREATE INDEX IF NOT EXISTS question_item ON question(item_id);
CREATE INDEX IF NOT EXISTS question_node ON question(plan_node_id);
CREATE TRIGGER IF NOT EXISTS question_version_follows_decision
AFTER UPDATE ON decision
BEGIN
    UPDATE question SET version = version + 1 WHERE decision_id = NEW.id;
END;
"#;
