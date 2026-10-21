use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

use anyhow::Context;
use bus::*;
use chrono::{DateTime, Duration, Utc};
use rusqlite::{params, Connection, OptionalExtension};

pub use crate::actor::Actor;
pub use board_attachments::NewAttachment;
/// The column categories a card goes stale in.
pub(crate) use board_cards::is_active as ages_in;
pub use board_drafts::UnseenDraft;
pub use board_edit::{ItemEdit, MoveTo};
pub use board_import::{Created, ImportReport, Warned};
pub use board_items::{NewItem, Write};
pub use board_keeper::{KeeperCard, KeeperMark, TurnWork};
pub use board_proposals::BoardProposal;
pub use board_reads::BoardSnapshot;
pub use board_tx::BoardTx;
pub use card_questions::Asker;
pub use decisions::NewDecision;
pub use decisions_edit::DecisionEdit;
pub use decisions_list::DecisionFilter;
pub use overview::LatestSummary;
pub use owner_messages::{OwnerProof, OwnerVia};
pub use owner_threads::{Asked, OwnerQuestion};
pub use peer_grants::{GrantEnd, GrantRuling, PeerGrant};
pub use plan_gates::Spike;
pub use questions::{Anchor, NewQuestion, QuestionFilter, QuestionRow};
pub use quiesce::{NewQuiesce, Quiesce};
pub use release_cut::{LeaveOut, NewCut};
pub use release_life::NewReleaseTest;
pub use release_machines::{Runs, TesterMachine};
pub use release_work::OnCard;
pub use releases::NewRelease;
pub use routines::RoutineLimits;
pub use runs::NewRun;
pub use signals::NewSignal;
pub use team::OpenWork;
pub use trash_orders::TrashOrder;
pub use usage::{Tokens, UsageEntry, UsageRow};
pub use usage_settings::{StoredBudget, UsagePrice};
pub use workers::NewWorker;

pub mod agent_talk;
mod artifact_uses;
mod board;
mod board_admin;
mod board_attachments;
mod board_cards;
mod board_columns;
mod board_drafts;
mod board_edit;
mod board_flow;
#[cfg(test)]
mod board_flow_tests;
mod board_import;
#[cfg(test)]
mod board_import_tests;
mod board_items;
mod board_keeper;
mod board_links;
mod board_moves;
mod board_notes;
mod board_proposals;
mod board_ranks;
mod board_reads;
#[cfg(test)]
mod board_schema_tests;
#[cfg(test)]
mod board_tests;
mod board_tx;
mod boards;
#[cfg(test)]
mod boards_migration_tests;
mod bot_runtime;
mod bots;
mod card_questions;
pub mod card_types;
mod card_worktrees;
pub use card_worktrees::CardWorktree;
#[cfg(test)]
mod card_types_tests;
pub mod check_jobs;
mod checks;
#[cfg(test)]
mod checks_tests;
mod cleanup;
mod cleanup_retention;
pub mod comments;
mod conversations;
pub mod dashboard;
mod decision_threads;
mod decisions;
mod decisions_edit;
mod decisions_list;
#[cfg(test)]
mod decisions_tests;
mod deliveries;
mod devices;
mod guardrails;
mod meetings;
pub mod merge_queue;
pub mod metrics;
mod migrate;
mod migrate_columns;
mod overview;
mod owner_actions;
mod owner_answers;
mod owner_messages;
mod owner_threads;
#[cfg(test)]
mod owner_threads_tests;
mod peer_grants;
mod peers;
mod permissions;
mod plan;
mod plan_gates;
mod plan_kickoff;
mod questions;
pub use plan_kickoff::NodeCard;
#[cfg(test)]
mod plan_tests;
#[cfg(test)]
mod platform_labels_migration_tests;
mod pr_changes;
mod project_links;
mod project_repos;
mod projects;
pub mod prs;
#[cfg(test)]
mod prs_tests;
mod quiesce;
mod release_cut;
mod release_life;
mod release_machines;
mod release_plans;
mod release_work;
mod releases;
#[cfg(test)]
mod releases_tests;
mod research_columns;
#[cfg(test)]
mod research_columns_tests;
pub mod review_settings;
pub mod reviews;
mod revisions;
#[cfg(test)]
mod roles_migration_tests;
mod routines;
mod runs;
mod runs_state;
#[cfg(test)]
mod runs_tests;
mod settings;
mod signals;
mod tags;
#[cfg(test)]
mod tags_tests;
mod task_views;
mod tasks;
mod team;
#[cfg(test)]
mod tests;
mod trash_orders;
mod typed_prompts;
pub mod usage;
pub mod usage_settings;
mod workers;
#[cfg(test)]
mod workers_tests;

/// Handle to the SQLite bus. All access is serialized through one connection;
/// statements are short so contention stays negligible at this scale.
#[derive(Clone)]
pub struct Db {
    conn: Arc<Mutex<Connection>>,
    /// PR changes, published after the transaction that made them (H-273).
    pub pr_feed: crate::prs::feed::PrFeed,
}

fn ts(t: DateTime<Utc>) -> String {
    t.to_rfc3339()
}

fn parse_ts(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s)
        .map(|d| d.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}

/// The schema version a database has stored: how many of `MIGRATIONS` it
/// has run, 0 before the first.
pub fn schema_version(conn: &Connection) -> anyhow::Result<usize> {
    let has_meta: i64 = conn.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='meta'",
        [],
        |r| r.get(0),
    )?;
    if has_meta == 0 {
        return Ok(0);
    }
    let stored = conn
        .query_row(
            "SELECT value FROM meta WHERE key='schema_version'",
            [],
            |r| r.get::<_, String>(0),
        )
        .optional()?;
    Ok(stored.and_then(|v| v.parse().ok()).unwrap_or(0))
}

impl Db {
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path).with_context(|| format!("open {}", path.display()))?;
        Self::init(conn)
    }

    pub fn open_in_memory() -> anyhow::Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> anyhow::Result<Self> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "busy_timeout", 5_000)?;
        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
            pr_feed: crate::prs::feed::PrFeed::default(),
        };
        db.migrate()?;
        db.backfill_dir_names()?;
        Ok(db)
    }

    fn lock(&self) -> MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Give pre-migration-3 bots and pre-migration-7 projects the directory
    /// names the schema now requires.
    ///
    /// Both migrations can only default the column to `''`, and an empty
    /// `dir_name` would make `paths::bot_dir` resolve to the shared `bots/`
    /// directory — so every upgraded bot would appear to share one workspace.
    /// The stored `workspace_path` already encodes the real directory, so
    /// recover it from there and fall back to deriving it from the name.
    ///
    /// A project's directory has always been `names::dir_name(name)`, so
    /// deriving it is exact rather than a fallback.
    fn backfill_dir_names(&self) -> anyhow::Result<()> {
        self.backfill_project_dir_names()?;
        let conn = self.lock();
        let rows: Vec<(String, String, String)> = conn
            .prepare("SELECT id, name, workspace_path FROM bot WHERE dir_name = ''")?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<Result<_, _>>()?;
        for (id, name, workspace_path) in rows {
            let from_path = Path::new(&workspace_path)
                .parent()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().to_string())
                .filter(|n| !n.is_empty());
            let dir_name = from_path.unwrap_or_else(|| bus::names::dir_name(&name));
            conn.execute(
                "UPDATE bot SET dir_name = ?2 WHERE id = ?1",
                params![id, dir_name],
            )?;
        }
        Ok(())
    }

    fn backfill_project_dir_names(&self) -> anyhow::Result<()> {
        let conn = self.lock();
        let rows: Vec<(String, String)> = conn
            .prepare("SELECT id, name FROM project WHERE dir_name = ''")?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<_, _>>()?;
        for (id, name) in rows {
            conn.execute(
                "UPDATE project SET dir_name = ?2 WHERE id = ?1",
                params![id, bus::names::dir_name(&name)],
            )?;
        }
        Ok(())
    }

    fn migrate(&self) -> anyhow::Result<()> {
        migrate::migrate(&mut self.lock())?;
        // What the 0.21 card types migration leaves to the daemon.
        self.backfill_card_type_templates()?;
        self.backfill_research_columns()
    }

    pub fn integrity_check(&self) -> anyhow::Result<bool> {
        let conn = self.lock();
        let res: String = conn.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
        Ok(res == "ok")
    }

    // ---- retention ----

    /// Delete old rows per retention policy. Message deletion cascades into
    /// the FTS index via triggers. Returns (messages, deliveries, runs) pruned.
    ///
    /// Signals go last and only when no surviving run points at one, so a run
    /// kept for history never loses the record of what caused it.
    pub fn prune(
        &self,
        message_days: i64,
        delivery_days: i64,
        run_days: i64,
        signal_days: i64,
    ) -> anyhow::Result<(usize, usize, usize)> {
        let mut conn = self.lock();
        let tx = conn.transaction()?;
        let cutoff = |days: i64| ts(now() - Duration::days(days.max(1)));
        let runs = tx.execute(
            "DELETE FROM routine_run WHERE state IN ('succeeded', 'failed', 'skipped', 'cancelled')
             AND scheduled_for < ?1",
            params![cutoff(run_days)],
        )?;
        let deliveries = {
            tx.execute(
                "DELETE FROM inbox WHERE delivery_id IN
                   (SELECT id FROM delivery WHERE state IN ('acknowledged', 'delivered', 'failed')
                    AND created_at < ?1)",
                params![cutoff(delivery_days)],
            )?;
            tx.execute(
                "DELETE FROM delivery WHERE state IN ('acknowledged', 'delivered', 'failed')
                 AND created_at < ?1",
                params![cutoff(delivery_days)],
            )?
        };
        let messages = {
            // Only messages with no remaining delivery/task references.
            tx.execute(
                "DELETE FROM task WHERE state != 'open' AND created_at < ?1
                 AND id NOT IN (SELECT source_task_id FROM decision
                                 WHERE source_task_id IS NOT NULL)",
                params![cutoff(message_days)],
            )?;
            tx.execute(
                "DELETE FROM task_card WHERE task_id NOT IN (SELECT id FROM task)",
                [],
            )?;
            tx.execute(
                "DELETE FROM task_release WHERE task_id NOT IN (SELECT id FROM task)",
                [],
            )?;
            tx.execute(
                "UPDATE message SET ref_message_id = NULL WHERE ref_message_id IN
                   (SELECT id FROM message WHERE created_at < ?1
                    AND id NOT IN (SELECT message_id FROM delivery)
                    AND id NOT IN (SELECT origin_message_id FROM task))",
                params![cutoff(message_days)],
            )?;
            // Decisions are never retained away, so neither is the exchange a
            // decision cites: a registry entry whose context has been pruned
            // is a ruling nobody can re-read the reason for.
            tx.execute(
                "DELETE FROM message WHERE created_at < ?1
                 AND id NOT IN (SELECT message_id FROM delivery)
                 AND id NOT IN (SELECT origin_message_id FROM task)
                 AND id NOT IN (SELECT ref_message_id FROM message WHERE ref_message_id IS NOT NULL)
                 AND id NOT IN (SELECT source_message_id FROM decision
                                 WHERE source_message_id IS NOT NULL)",
                params![cutoff(message_days)],
            )?
        };
        tx.commit()?;
        drop(conn);
        self.prune_signals(now() - Duration::days(signal_days.max(1)))?;
        Ok((messages, deliveries, runs))
    }
}
