//! Plan storage, part 2 (0.21.0): a node's gate, its documents, its history,
//! and the spikes its research waits on.

use bus::now;
use rusqlite::{params, Connection, OptionalExtension};

use super::board::{from_json, from_text, parse_at, to_text};
use super::board_cards::cards_in;
use super::board_tx::BoardTx;
use super::ts;
use crate::kanban::model::{ColumnCategory, ItemCard, LinkKind};
use crate::plan::model::{Doc, DocKind, Event, Gate, Stage};

pub(super) fn docs_in(conn: &Connection, node_id: &str) -> anyhow::Result<Vec<Doc>> {
    let docs = conn
        .prepare(
            "SELECT kind, path, approved_sha, approved_at, item_id FROM plan_doc
             WHERE node_id = ?1 ORDER BY CASE kind
               WHEN 'goal' THEN 0 WHEN 'research' THEN 1 WHEN 'prd' THEN 2
               WHEN 'design' THEN 3 WHEN 'hld' THEN 4 ELSE 5 END, path",
        )?
        .query_map(params![node_id], |r| {
            Ok(Doc {
                kind: from_text(r.get(0)?)?,
                path: r.get(1)?,
                approved_sha: r.get(2)?,
                approved_at: r.get::<_, Option<String>>(3)?.map(parse_at),
                changed_since_approval: false,
                item_id: r.get(4)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(docs)
}

pub(super) fn gate_in(conn: &Connection, node_id: &str) -> anyhow::Result<Option<Gate>> {
    Ok(conn
        .query_row(
            "SELECT stage, doc_path, sha, pr_number, submitted_by, submitted_at, note, children,
                    round
             FROM plan_gate WHERE node_id = ?1",
            params![node_id],
            |r| {
                Ok(Gate {
                    stage: from_text(r.get(0)?)?,
                    doc_path: r.get(1)?,
                    sha: r.get(2)?,
                    pr_number: r.get(3)?,
                    submitted_by: r.get(4)?,
                    submitted_at: parse_at(r.get(5)?),
                    note: r.get(6)?,
                    children: from_json(r.get(7)?)?,
                    decision_id: None,
                    round: r.get(8)?,
                })
            },
        )
        .optional()?)
}

/// A spike of a node's research: whether it is done, and its documents.
#[derive(Debug, Clone, PartialEq)]
pub struct Spike {
    pub id: String,
    pub done: bool,
    /// Its artifact links and its attachments still on it.
    pub docs: Vec<String>,
}

impl BoardTx<'_> {
    /// Replaces the node's gate.
    pub fn set_plan_gate(&self, node_id: &str, gate: &Gate) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO plan_gate(node_id, stage, doc_path, sha, pr_number,
                 submitted_by, submitted_at, note, children, round)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                node_id,
                to_text(&gate.stage),
                gate.doc_path,
                gate.sha,
                gate.pr_number,
                gate.submitted_by,
                ts(gate.submitted_at),
                gate.note,
                serde_json::to_string(&gate.children)?,
                gate.round
            ],
        )?;
        Ok(())
    }

    pub fn clear_plan_gate(&self, node_id: &str) -> anyhow::Result<()> {
        self.conn
            .execute("DELETE FROM plan_gate WHERE node_id = ?1", params![node_id])?;
        Ok(())
    }

    /// How many times the node was submitted at `stage`: the next round is
    /// one more.
    pub fn plan_rounds(&self, node_id: &str, stage: Stage) -> anyhow::Result<u32> {
        Ok(self.conn.query_row(
            "SELECT count(*) FROM plan_event WHERE node_id = ?1 AND kind = 'submitted'
             AND stage = ?2",
            params![node_id, to_text(&stage)],
            |r| r.get(0),
        )?)
    }

    /// Records the node's document; a document already there keeps its
    /// approval until the next one.
    pub fn upsert_plan_doc(
        &self,
        node_id: &str,
        kind: DocKind,
        path: &str,
        item_id: Option<&str>,
    ) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT INTO plan_doc(node_id, kind, path, item_id) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(node_id, kind, path) DO UPDATE SET item_id = excluded.item_id",
            params![node_id, to_text(&kind), path, item_id],
        )?;
        Ok(())
    }

    /// The owner approved the node's document at `sha` (none when the
    /// submission named no commit).
    pub fn approve_plan_doc(
        &self,
        node_id: &str,
        kind: DocKind,
        path: &str,
        sha: Option<&str>,
    ) -> anyhow::Result<()> {
        self.upsert_plan_doc(node_id, kind, path, None)?;
        self.conn.execute(
            "UPDATE plan_doc SET approved_sha = ?4, approved_at = ?5
             WHERE node_id = ?1 AND kind = ?2 AND path = ?3",
            params![node_id, to_text(&kind), path, sha, ts(now())],
        )?;
        Ok(())
    }

    pub fn record_plan_event(
        &self,
        node_id: &str,
        actor: &str,
        kind: &str,
        stage: Option<Stage>,
        note: Option<&str>,
    ) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT INTO plan_event(node_id, at, actor, kind, stage, note)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                node_id,
                ts(now()),
                actor,
                kind,
                stage.map(|s| to_text(&s)),
                note
            ],
        )?;
        Ok(())
    }

    /// The node's history, oldest first.
    pub fn plan_events(&self, node_id: &str) -> anyhow::Result<Vec<Event>> {
        let events = self
            .conn
            .prepare(
                "SELECT at, actor, kind, stage, note FROM plan_event WHERE node_id = ?1
                 ORDER BY id",
            )?
            .query_map(params![node_id], |r| {
                Ok(Event {
                    at: parse_at(r.get(0)?),
                    actor: r.get(1)?,
                    kind: r.get(2)?,
                    // A stage a newer daemon wrote reads as none.
                    stage: r
                        .get::<_, Option<String>>(3)?
                        .and_then(|s| Stage::parse(&s)),
                    note: r.get(4)?,
                })
            })?
            .collect::<Result<_, _>>()?;
        Ok(events)
    }

    /// The project's node whose gate or documents are at `path` (0.22.0).
    pub fn plan_node_of_doc(&self, project_id: &str, path: &str) -> anyhow::Result<Option<String>> {
        Ok(self
            .conn
            .query_row(
                "SELECT n.id FROM plan_node n
                  WHERE n.project_id = ?1
                    AND (EXISTS (SELECT 1 FROM plan_gate g WHERE g.node_id = n.id AND g.doc_path = ?2)
                      OR EXISTS (SELECT 1 FROM plan_doc d WHERE d.node_id = n.id AND d.path = ?2))
                  ORDER BY n.updated_at DESC LIMIT 1",
                params![project_id, path],
                |r| r.get(0),
            )
            .optional()?)
    }

    /// The live cards of a node, in column then rank order.
    pub fn plan_cards(&self, project_id: &str, node_id: &str) -> anyhow::Result<Vec<ItemCard>> {
        cards_in(
            self.conn,
            project_id,
            "AND i.plan_node_id = ?2",
            Some(node_id),
        )
    }

    /// The node's live spikes, each with whether it is done and its
    /// documents: an artifact link, or an attachment not removed.
    pub fn plan_spikes(&self, node_id: &str) -> anyhow::Result<Vec<Spike>> {
        let rows: Vec<(String, String)> = self
            .conn
            .prepare(
                "SELECT id, category FROM item WHERE plan_node_id = ?1 AND type = 'spike'
                 AND state = 'live' ORDER BY seq",
            )?
            .query_map(params![node_id], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<_, _>>()?;
        let mut spikes = Vec::with_capacity(rows.len());
        for (id, category) in rows {
            let mut docs: Vec<String> = self
                .item_links(&id)?
                .into_iter()
                .filter(|l| l.kind == LinkKind::Artifact)
                .map(|l| l.target)
                .collect();
            docs.extend(
                self.attachments(&id)?
                    .into_iter()
                    .filter(|a| !a.removed)
                    .map(|a| match (a.path.is_empty(), a.url.is_empty()) {
                        (false, _) => a.path,
                        (true, false) => a.url,
                        (true, true) => a.name,
                    }),
            );
            spikes.push(Spike {
                done: category == to_text(&ColumnCategory::Done),
                id,
                docs,
            });
        }
        Ok(spikes)
    }

    /// The project's task template, for the cards an approval makes.
    pub fn task_template(&self, project_id: &str) -> anyhow::Result<Option<serde_json::Value>> {
        Ok(super::board::item_template(self.conn, project_id, "task")?)
    }
}
