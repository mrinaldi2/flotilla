//! Reading the plan: a project's whole tree and one node with its history
//! and cards. A read first records research its spikes closed, so what it
//! shows is current (`research::settle`).

use super::model::{Doc, Event, Node};
use super::{docs, research, Planner};
use crate::decisions::not_found;
use crate::kanban::model::ItemCard;

/// A project's plan, as `plan_get` answers it.
#[derive(Debug, Clone, PartialEq)]
pub struct Tree {
    pub project_id: String,
    pub enabled: bool,
    pub has_repo: bool,
    /// Parents before children; none while planning is off.
    pub nodes: Vec<Node>,
    pub guidelines: Vec<Doc>,
}

/// One node, as `plan_node_get` answers it.
#[derive(Debug, Clone, PartialEq)]
pub struct Detail {
    pub node: Node,
    pub history: Vec<Event>,
    pub cards: Vec<ItemCard>,
}

impl Planner<'_> {
    pub fn tree(&self, project_id: &str) -> anyhow::Result<Tree> {
        let db = &self.app.db;
        let live = db
            .get_project(project_id)?
            .is_some_and(|p| p.deleted_at.is_none());
        if !live || !self.sees(project_id) {
            return Err(not_found(format!("no project {project_id}")));
        }
        let has_repo = db.project_repo(project_id)?.is_some();
        let enabled = self.enabled(project_id)?;
        if enabled {
            self.settle(project_id)?;
        }
        let nodes = match enabled {
            true => db.board_read(|t| t.plan_nodes(project_id))?,
            false => Vec::new(),
        };
        let guidelines = match enabled && has_repo {
            true => docs::guidelines(self.app, project_id),
            false => Vec::new(),
        };
        Ok(Tree {
            project_id: project_id.to_string(),
            enabled,
            has_repo,
            nodes,
            guidelines,
        })
    }

    /// The node with its history and its own cards; `None` when there is
    /// no such node this caller may see. Its approved documents say whether
    /// the default branch has changed them since.
    pub fn detail(&self, id: &str) -> anyhow::Result<Option<Detail>> {
        let Some(project_id) = self.project_of(id)?.filter(|p| self.sees(p)) else {
            return Ok(None);
        };
        if self.enabled(&project_id)? {
            self.settle(&project_id)?;
        }
        let read = self.app.db.board_read(|t| {
            let Some(node) = t.plan_node(id)? else {
                return Ok(None);
            };
            let history = t.plan_events(id)?;
            let cards = t.plan_cards(&project_id, id)?;
            Ok(Some(Detail {
                node,
                history,
                cards,
            }))
        })?;
        let Some(mut detail) = read else {
            return Ok(None);
        };
        if detail.node.docs.iter().any(|d| d.approved_sha.is_some()) {
            if let Some((cache, head)) = docs::head(self.app, &project_id, true) {
                for doc in &mut detail.node.docs {
                    doc.changed_since_approval = docs::changed_since_approval(&cache, &head, doc);
                }
            }
        }
        Ok(Some(detail))
    }

    fn enabled(&self, project_id: &str) -> anyhow::Result<bool> {
        Ok(self
            .app
            .db
            .board_read(|t| t.plan_settings(project_id))?
            .is_some_and(|s| s.enabled))
    }

    /// Record research its spikes closed, and nodes whose work shipped,
    /// since the plan was last read.
    pub fn settle(&self, project_id: &str) -> anyhow::Result<()> {
        self.run(|t, wrote| {
            research::settle(t, wrote, project_id)?;
            super::closure::settle(t, wrote, project_id)?;
            wrote.look = t
                .plan_nodes(project_id)?
                .into_iter()
                .filter(|n| !n.stage.is_final())
                .map(|n| n.id)
                .collect();
            Ok(())
        })?;
        Ok(())
    }
}
