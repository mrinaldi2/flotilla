//! The owner's ruling on a node's gate (`plan_rule`). Approved, the node
//! moves on and the submission's children are made: epics from a PRD,
//! features from an HLD, task cards on the initiative's board from an LLD.
//! Sent back (or, for a proposal, declined) it needs the owner's note.

use bus::contract::board as c;

use super::model::{Child, Gate, Kind, NewNode, Node, ResearchState, Stage};
use super::{changed, owner_only, refused, research, Planned, Planner, Step, Wrote};
use crate::db::{BoardTx, NewItem};
use crate::kanban::defaults::key_for;
use crate::kanban::model::{ItemType, Priority};
use crate::kanban::presets::Preset;

impl Planner<'_> {
    pub fn rule(&self, r: &c::PlanRule) -> anyhow::Result<Planned> {
        let by = self.by();
        let note = r.note.as_deref().map(str::trim).filter(|n| !n.is_empty());
        self.write(|t, wrote| {
            let node = match self.open_node(t, &r.id)? {
                Ok(node) => node,
                Err(unmet) => return Ok(Step::Refused(vec![unmet])),
            };
            if node.version != r.expected_version {
                return Ok(Step::Conflict(Box::new(node)));
            }
            if !self.actor.is_owner() {
                return Ok(Step::Refused(vec![owner_only()]));
            }
            let Some(gate) = node.gate.clone().filter(|g| g.stage == node.stage) else {
                return Ok(refused(
                    "plan.no_gate",
                    format!("Nothing of {} waits for your ruling.", node.id),
                ));
            };
            if !r.approve {
                let Some(note) = note else {
                    return Ok(refused(
                        "plan.note",
                        "Say what should change: the lead reads your note.",
                    ));
                };
                t.clear_plan_gate(&node.id)?;
                if gate.stage == Stage::Proposed {
                    t.set_plan_stage(&node.id, Stage::Dropped)?;
                    let stage = Some(Stage::Dropped);
                    changed(t, wrote, &node.id, &by, "declined", stage, Some(note))?;
                } else {
                    let stage = Some(gate.stage);
                    changed(t, wrote, &node.id, &by, "sent_back", stage, Some(note))?;
                }
                return Ok(Step::Done(node.id));
            }
            if let Some(i) = r
                .leave_out
                .iter()
                .find(|i| **i as usize >= gate.children.len())
            {
                return Ok(refused(
                    "plan.leave_out",
                    format!("The submission has no child {i} to leave out."),
                ));
            }
            let kept: Vec<&Child> = gate
                .children
                .iter()
                .enumerate()
                .filter(|(i, _)| !r.leave_out.contains(&(*i as u32)))
                .map(|(_, child)| child)
                .collect();
            t.clear_plan_gate(&node.id)?;
            self.approve(t, wrote, &node, &gate, &kept, note)?;
            Ok(Step::Done(node.id))
        })
    }

    /// The node moves on from its gate, and its children are made.
    fn approve(
        &self,
        t: &BoardTx<'_>,
        wrote: &mut Wrote,
        node: &Node,
        gate: &Gate,
        kept: &[&Child],
        note: Option<&str>,
    ) -> anyhow::Result<()> {
        let by = self.by();
        if let (Some(kind), Some(path)) = (gate.stage.doc_kind(), gate.doc_path.as_deref()) {
            t.approve_plan_doc(&node.id, kind, path, gate.sha.as_deref())?;
        }
        let next = match gate.stage {
            Stage::Proposed => {
                let board = t.create_board(
                    &node.project_id,
                    &node.title,
                    &key_for(&node.title),
                    Preset::SimpleReview,
                )?;
                wrote.boards.push(board.id.clone());
                t.set_plan_board(&node.id, &board.id)?;
                t.set_plan_research(&node.id, ResearchState::Open, None)?;
                t.set_plan_stage(&node.id, Stage::Research)?;
                let stage = Some(Stage::Research);
                return changed(t, wrote, &node.id, &by, "accepted", stage, note);
            }
            Stage::Research => {
                let reason = gate.note.as_deref().unwrap_or_default();
                return research::skip(t, wrote, node, &by, reason);
            }
            Stage::Prd => {
                self.children(t, wrote, node, Kind::Epic, kept)?;
                Stage::Running
            }
            Stage::Design => Stage::Hld,
            Stage::Hld => {
                self.children(t, wrote, node, Kind::Feature, kept)?;
                Stage::Running
            }
            Stage::Lld => {
                self.tasks(t, wrote, node, kept)?;
                Stage::Build
            }
            other => anyhow::bail!("{} has a gate at {}", node.id, other.as_str()),
        };
        t.set_plan_stage(&node.id, next)?;
        changed(t, wrote, &node.id, &by, "approved", Some(gate.stage), note)
    }

    /// Epics of an initiative or features of an epic, on its board.
    fn children(
        &self,
        t: &BoardTx<'_>,
        wrote: &mut Wrote,
        node: &Node,
        kind: Kind,
        kept: &[&Child],
    ) -> anyhow::Result<()> {
        let by = self.by();
        let (stage, research) = match kind {
            Kind::Epic => (Stage::Research, Some(ResearchState::Open)),
            _ => (Stage::Lld, None),
        };
        for child in kept {
            let made = t.insert_plan_node(
                &NewNode {
                    project_id: &node.project_id,
                    kind,
                    parent_id: Some(&node.id),
                    title: &child.title,
                    summary: child.summary.as_deref().unwrap_or_default(),
                    stage,
                    due: None,
                    lead: None,
                    board_id: node.board_id.as_deref(),
                    research,
                },
                &by,
            )?;
            t.record_plan_event(&made.id, &by, "created", Some(stage), None)?;
            wrote.node(&made.id);
            wrote.created.push(made.id);
        }
        Ok(())
    }

    /// A feature's tasks: one card each on its initiative's board.
    fn tasks(
        &self,
        t: &BoardTx<'_>,
        wrote: &mut Wrote,
        node: &Node,
        kept: &[&Child],
    ) -> anyhow::Result<()> {
        for child in kept {
            let item = t.create_item(
                &NewItem {
                    project_id: &node.project_id,
                    board_id: node.board_id.as_deref(),
                    item_type: ItemType::Task,
                    title: &child.title,
                    description: child.summary.as_deref().unwrap_or_default(),
                    size: None,
                    priority: Priority::P2,
                    labels: &[],
                    parent_id: None,
                    acceptance_criteria: &[],
                    plan_node_id: Some(&node.id),
                },
                &self.actor,
            )?;
            wrote.cards.push(item.id.clone());
            wrote.created.push(item.id);
        }
        Ok(())
    }
}
