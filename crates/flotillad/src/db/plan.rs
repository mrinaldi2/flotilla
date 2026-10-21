//! Plan storage, part 1 (0.21.0, migration 073): a project's planning
//! switch, its node ids, and its nodes. A node is read whole: its row, its
//! documents, its gate, its research and the count of its cards. Every
//! change runs inside a `BoardTx`, so an approval's children, cards and
//! boards commit with it.
//!
//! Node ids are `PL-<n>`, `n` from the project's own sequence. Ids are
//! unique across projects too, so a project whose `PL-<n>` another project
//! already holds takes `<KEY>-PL-<n>`, its board key as the prefix.

use bus::now;
use rusqlite::{params, Connection, OptionalExtension};

use super::board::{from_text, parse_at, settings_in, to_text};
use super::board_tx::BoardTx;
use super::ts;
use crate::kanban::model::ColumnCategory;
use crate::kanban::rank;
use crate::plan::model::{NewNode, Node, NodeEdit, Research, ResearchState, Settings, Stage};

const NODE_COLUMNS: &str = "id, project_id, kind, parent_id, title, summary, stage, due, lead, \
     board_id, research, research_closed_at, research_reason, rank, created_by, created_at, \
     updated_at, version";

fn node_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Node> {
    let research = match r.get::<_, Option<String>>(10)? {
        Some(state) => Some(Research {
            state: from_text(state)?,
            spikes: Vec::new(),
            closed_at: r.get::<_, Option<String>>(11)?.map(parse_at),
            skipped_reason: r.get(12)?,
        }),
        None => None,
    };
    Ok(Node {
        id: r.get(0)?,
        project_id: r.get(1)?,
        kind: from_text(r.get(2)?)?,
        parent_id: r.get(3)?,
        title: r.get(4)?,
        summary: r.get(5)?,
        stage: from_text(r.get(6)?)?,
        due: r.get(7)?,
        lead: r.get(8)?,
        board_id: r.get(9)?,
        docs: Vec::new(),
        research,
        gate: None,
        cards_total: 0,
        cards_done: 0,
        questions_open: 0,
        questions_blocking: 0,
        rank: r.get(13)?,
        created_by: r.get(14)?,
        created_at: parse_at(r.get(15)?),
        updated_at: parse_at(r.get(16)?),
        version: r.get(17)?,
    })
}

/// The node, whole, as it is in `conn`.
pub(super) fn node_in(conn: &Connection, id: &str) -> anyhow::Result<Option<Node>> {
    let Some(mut node) = conn
        .query_row(
            &format!("SELECT {NODE_COLUMNS} FROM plan_node WHERE id = ?1"),
            params![id],
            node_from_row,
        )
        .optional()?
    else {
        return Ok(None);
    };
    fill(conn, &mut node)?;
    Ok(Some(node))
}

/// A node's documents, gate, spikes and card counts.
fn fill(conn: &Connection, node: &mut Node) -> anyhow::Result<()> {
    node.docs = super::plan_gates::docs_in(conn, &node.id)?;
    node.gate = super::plan_gates::gate_in(conn, &node.id)?;
    if let Some(research) = node.research.as_mut() {
        research.spikes = conn
            .prepare(
                "SELECT id FROM item WHERE plan_node_id = ?1 AND type = 'spike'
                 AND state = 'live' ORDER BY seq",
            )?
            .query_map(params![node.id], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
    }
    // A node's progress is the cards that build it, not the lead's
    // kickoffs (its plan work, which the stage already shows).
    let (total, done): (u32, u32) = conn.query_row(
        "SELECT count(*), coalesce(sum(category = ?2), 0) FROM item
         WHERE plan_node_id = ?1 AND state = 'live' AND category != ?3
         AND labels NOT LIKE '%\"kickoff\"%'",
        params![
            node.id,
            to_text(&ColumnCategory::Done),
            to_text(&ColumnCategory::Cancelled)
        ],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    (node.cards_total, node.cards_done) = (total, done);
    (node.questions_open, node.questions_blocking) =
        super::questions::open_on_node(conn, &node.id)?;
    Ok(())
}

impl BoardTx<'_> {
    /// The project's planning switch; `None` before it was ever turned on.
    pub fn plan_settings(&self, project_id: &str) -> anyhow::Result<Option<Settings>> {
        Ok(self
            .conn
            .query_row(
                "SELECT enabled, next_seq, research_gate FROM plan_settings WHERE project_id = ?1",
                params![project_id],
                |r| {
                    Ok(Settings {
                        enabled: r.get(0)?,
                        next_seq: r.get(1)?,
                        research_gate: r.get(2)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn set_plan_enabled(&self, project_id: &str, enabled: bool) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT INTO plan_settings(project_id, enabled, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(project_id) DO UPDATE
               SET enabled = excluded.enabled, updated_at = excluded.updated_at",
            params![project_id, enabled, ts(now())],
        )?;
        Ok(())
    }

    /// Whether the project's research waits for the owner's review of its
    /// findings (0.24.0): on until the owner turns it off.
    pub fn plan_research_gate(&self, project_id: &str) -> anyhow::Result<bool> {
        Ok(self
            .plan_settings(project_id)?
            .is_none_or(|s| s.research_gate))
    }

    pub fn set_plan_research_gate(&self, project_id: &str, on: bool) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT INTO plan_settings(project_id, research_gate, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(project_id) DO UPDATE
               SET research_gate = excluded.research_gate, updated_at = excluded.updated_at",
            params![project_id, on, ts(now())],
        )?;
        Ok(())
    }

    pub fn plan_node(&self, id: &str) -> anyhow::Result<Option<Node>> {
        node_in(self.conn, id)
    }

    /// Every node of the project, parents before children, siblings by rank.
    pub fn plan_nodes(&self, project_id: &str) -> anyhow::Result<Vec<Node>> {
        let rows: Vec<Node> = self
            .conn
            .prepare(&format!(
                "SELECT {NODE_COLUMNS} FROM plan_node WHERE project_id = ?1 ORDER BY rank, id"
            ))?
            .query_map(params![project_id], node_from_row)?
            .collect::<Result<_, _>>()?;
        let mut ordered = Vec::with_capacity(rows.len());
        order_under(None, &rows, &mut ordered);
        // A node whose parent is missing still shows, last.
        for node in rows {
            if !ordered.iter().any(|n: &Node| n.id == node.id) {
                ordered.push(node);
            }
        }
        for node in &mut ordered {
            fill(self.conn, node)?;
        }
        Ok(ordered)
    }

    /// The node and every node under it, the node first.
    pub fn plan_subtree(&self, id: &str) -> anyhow::Result<Vec<String>> {
        let mut out = vec![id.to_string()];
        let mut i = 0;
        while i < out.len() {
            let children: Vec<String> = self
                .conn
                .prepare("SELECT id FROM plan_node WHERE parent_id = ?1 ORDER BY rank, id")?
                .query_map(params![out[i]], |r| r.get(0))?
                .collect::<Result<_, _>>()?;
            out.extend(children);
            i += 1;
        }
        Ok(out)
    }

    /// A new node, ranked last among its siblings, with its next id.
    pub fn insert_plan_node(&self, new: &NewNode<'_>, actor: &str) -> anyhow::Result<Node> {
        let id = self.next_plan_id(new.project_id)?;
        let last: Option<String> = self.conn.query_row(
            "SELECT max(rank) FROM plan_node WHERE project_id = ?1 AND parent_id IS ?2",
            params![new.project_id, new.parent_id],
            |r| r.get(0),
        )?;
        let rank = rank::between(last.as_deref(), None)
            .ok_or_else(|| anyhow::anyhow!("no rank after {last:?}"))?;
        let at = ts(now());
        self.conn.execute(
            &format!(
                "INSERT INTO plan_node({NODE_COLUMNS})
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, NULL, NULL, ?12, ?13,
                         ?14, ?14, 1)"
            ),
            params![
                id,
                new.project_id,
                to_text(&new.kind),
                new.parent_id,
                new.title,
                new.summary,
                to_text(&new.stage),
                new.due,
                new.lead,
                new.board_id,
                new.research.map(|r| to_text(&r)),
                rank,
                actor,
                at
            ],
        )?;
        Ok(node_in(self.conn, &id)?.expect("just inserted"))
    }

    /// Take the node's next version, or the node as it is when `expected`
    /// is stale. `None` for `expected` takes it whatever it is.
    pub fn claim_plan_node(&self, id: &str, expected: Option<u64>) -> anyhow::Result<Option<Node>> {
        let claimed = self.conn.execute(
            "UPDATE plan_node SET version = version + 1, updated_at = ?3
             WHERE id = ?1 AND (?2 IS NULL OR version = ?2)",
            params![id, expected, ts(now())],
        )?;
        if claimed == 1 {
            return Ok(None);
        }
        node_in(self.conn, id)?
            .map(Some)
            .ok_or_else(|| anyhow::anyhow!("no plan node {id}"))
    }

    pub fn set_plan_stage(&self, id: &str, stage: Stage) -> anyhow::Result<()> {
        self.conn.execute(
            "UPDATE plan_node SET stage = ?2 WHERE id = ?1",
            params![id, to_text(&stage)],
        )?;
        Ok(())
    }

    /// The node's research: its state, when it closed, and why it was
    /// skipped.
    pub fn set_plan_research(
        &self,
        id: &str,
        state: ResearchState,
        reason: Option<&str>,
    ) -> anyhow::Result<()> {
        let closed_at = (state == ResearchState::Closed).then(|| ts(now()));
        self.conn.execute(
            "UPDATE plan_node SET research = ?2, research_closed_at = ?3, research_reason = ?4
             WHERE id = ?1",
            params![id, to_text(&state), closed_at, reason],
        )?;
        Ok(())
    }

    pub fn set_plan_board(&self, id: &str, board_id: &str) -> anyhow::Result<()> {
        self.conn.execute(
            "UPDATE plan_node SET board_id = ?2 WHERE id = ?1",
            params![id, board_id],
        )?;
        Ok(())
    }

    /// Applies `edit`; returns the names of the fields that changed.
    pub fn edit_plan_node(
        &self,
        id: &str,
        edit: &NodeEdit<'_>,
    ) -> anyhow::Result<Vec<&'static str>> {
        let before = node_in(self.conn, id)?.ok_or_else(|| anyhow::anyhow!("no plan node {id}"))?;
        let mut changed = Vec::new();
        let mut set = |column: &'static str, value: Option<&str>| -> anyhow::Result<()> {
            self.conn.execute(
                &format!("UPDATE plan_node SET {column} = ?2 WHERE id = ?1"),
                params![id, value],
            )?;
            changed.push(column);
            Ok(())
        };
        if let Some(title) = edit.title.filter(|t| *t != before.title) {
            set("title", Some(title))?;
        }
        if let Some(summary) = edit.summary.filter(|s| *s != before.summary) {
            set("summary", Some(summary))?;
        }
        if let Some(due) = edit.due.filter(|d| *d != before.due.as_deref()) {
            set("due", due)?;
        }
        if let Some(lead) = edit.lead.filter(|l| *l != before.lead.as_deref()) {
            set("lead", lead)?;
        }
        Ok(changed)
    }

    /// The project's next node id: `PL-<n>`, or `<KEY>-PL-<n>` when another
    /// project holds `PL-<n>`.
    fn next_plan_id(&self, project_id: &str) -> anyhow::Result<String> {
        let key = settings_in(self.conn, project_id)?.map(|s| s.key);
        loop {
            let seq: u32 = self.conn.query_row(
                "SELECT next_seq FROM plan_settings WHERE project_id = ?1",
                params![project_id],
                |r| r.get(0),
            )?;
            self.conn.execute(
                "UPDATE plan_settings SET next_seq = next_seq + 1 WHERE project_id = ?1",
                params![project_id],
            )?;
            let plain = format!("PL-{seq}");
            let keyed = key.as_ref().map(|k| format!("{k}-PL-{seq}"));
            for id in std::iter::once(plain).chain(keyed) {
                if !self.plan_id_taken(&id)? {
                    return Ok(id);
                }
            }
        }
    }

    fn plan_id_taken(&self, id: &str) -> anyhow::Result<bool> {
        Ok(self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM plan_node WHERE id = ?1)",
            params![id],
            |r| r.get(0),
        )?)
    }
}

/// `nodes` under `parent`, depth first, each level in the rank order the
/// query gave.
fn order_under(parent: Option<&str>, nodes: &[Node], out: &mut Vec<Node>) {
    for node in nodes.iter().filter(|n| n.parent_id.as_deref() == parent) {
        out.push(node.clone());
        order_under(Some(&node.id), nodes, out);
    }
}
