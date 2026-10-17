//! What the lead puts before the owner: a gate's document (`plan_submit`)
//! and a request to skip research (`plan_research_skip`), which the owner's
//! own skip does at once.

use bus::contract::board as c;

use super::model::{Child, Gate, Node, ResearchState, Stage};
use super::research::{close_if_done, skip, still_open};
use super::{changed, refused, Planned, Planner, Step, Wrote};
use crate::db::BoardTx;
use crate::kanban::guards::unmet;

/// Why a node's research doesn't let it move on: open, with its spikes'
/// state. `None` when it is closed or skipped, or the node has none.
fn research_open(t: &BoardTx<'_>, wrote: &mut Wrote, node: &Node) -> anyhow::Result<Option<Step>> {
    if !node
        .research
        .as_ref()
        .is_some_and(|r| r.state == ResearchState::Open)
    {
        return Ok(None);
    }
    if close_if_done(t, wrote, node)? {
        return Ok(None);
    }
    let text =
        still_open(&t.plan_spikes(&node.id)?).unwrap_or_else(|| "Research is open.".to_string());
    Ok(Some(Step::Refused(vec![unmet(
        "plan.research_open",
        text,
        Some("Finish the spikes with their documents, or ask the owner to skip research."),
    )])))
}

impl Planner<'_> {
    /// The lead submitting the document of the node's gate stage. Research
    /// must be over; the commit defaults to the default branch's head.
    pub fn submit(&self, r: &c::PlanSubmit) -> anyhow::Result<Planned> {
        let Some(project_id) = self.project_of(&r.id)? else {
            return Ok(self.missing(&r.id));
        };
        let doc_path = super::docs::check_doc_path(&r.doc_path)?.to_string();
        let sha = match r.sha.as_deref().filter(|s| !s.trim().is_empty()) {
            Some(sha) => Some(super::docs::check_sha(sha)?.to_string()),
            None => super::docs::head(self.app, &project_id, true).map(|(_, sha)| sha),
        };
        let children: Vec<Child> = r
            .children
            .iter()
            .map(|child| Child {
                title: child.title.trim().to_string(),
                summary: child
                    .summary
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string),
            })
            .collect();
        if children.iter().any(|c| c.title.is_empty()) {
            return Ok(Planned {
                outcome: super::Outcome::Refused(vec![unmet(
                    "plan.children",
                    "Every proposed child needs a title.",
                    None,
                )]),
                created: Vec::new(),
            });
        }
        let project_lead = self.project_lead(&project_id)?;
        let by = self.by();
        self.write(|t, wrote| {
            let node = match self.open_node(t, &r.id)? {
                Ok(node) => node,
                Err(unmet) => return Ok(Step::Refused(vec![unmet])),
            };
            if node.version != r.expected_version {
                return Ok(Step::Conflict(Box::new(node)));
            }
            if !self.leads(Some(&node), project_lead.as_deref()) {
                return Ok(refused(
                    "plan.lead_only",
                    "Only the node's lead or the owner submits its documents.",
                ));
            }
            if let Some(open) = research_open(t, wrote, &node)? {
                return Ok(open);
            }
            // Research that just closed moved the node on.
            let node = t.plan_node(&node.id)?.expect("read above");
            if !node.kind.gates_at(node.stage) {
                return Ok(refused(
                    "plan.stage",
                    format!(
                        "{} is in {}, which isn't a gate of a {}.",
                        node.id,
                        node.stage.as_str(),
                        node.kind.as_str()
                    ),
                ));
            }
            let round = t.plan_rounds(&node.id, node.stage)? + 1;
            let note = r.note.as_deref().map(str::trim).filter(|n| !n.is_empty());
            t.set_plan_gate(
                &node.id,
                &Gate {
                    stage: node.stage,
                    doc_path: Some(doc_path.clone()),
                    sha: sha.clone(),
                    pr_number: r.pr_number,
                    submitted_by: by.clone(),
                    submitted_at: bus::now(),
                    note: note.map(str::to_string),
                    children: children.clone(),
                    decision_id: None,
                    round,
                },
            )?;
            if let Some(kind) = node.stage.doc_kind() {
                t.upsert_plan_doc(&node.id, kind, &doc_path, None)?;
            }
            changed(t, wrote, &node.id, &by, "submitted", Some(node.stage), note)?;
            Ok(Step::Done(node.id))
        })
    }

    /// Skip a node's research: the owner's skip moves it on at once; the
    /// lead's asks the owner, as a gate at stage RESEARCH.
    pub fn research_skip(&self, r: &c::PlanResearchSkip) -> anyhow::Result<Planned> {
        let Some(project_id) = self.project_of(&r.id)? else {
            return Ok(self.missing(&r.id));
        };
        let project_lead = self.project_lead(&project_id)?;
        let reason = r.reason.trim();
        let by = self.by();
        self.write(|t, wrote| {
            let node = match self.open_node(t, &r.id)? {
                Ok(node) => node,
                Err(unmet) => return Ok(Step::Refused(vec![unmet])),
            };
            if node.version != r.expected_version {
                return Ok(Step::Conflict(Box::new(node)));
            }
            if !self.leads(Some(&node), project_lead.as_deref()) {
                return Ok(refused(
                    "plan.lead_only",
                    "Only the node's lead or the owner asks to skip its research.",
                ));
            }
            let open = node
                .research
                .as_ref()
                .is_some_and(|r| r.state == ResearchState::Open);
            if node.stage != Stage::Research || !open {
                return Ok(refused(
                    "plan.research_state",
                    format!("{}'s research isn't open.", node.id),
                ));
            }
            if reason.is_empty() {
                return Ok(refused("plan.reason", "Say why research can be skipped."));
            }
            if self.actor.is_owner() {
                t.clear_plan_gate(&node.id)?;
                skip(t, wrote, &node, &by, reason)?;
                return Ok(Step::Done(node.id));
            }
            if node.gate.is_some() {
                return Ok(refused(
                    "plan.gate_waiting",
                    format!("{} already waits for the owner.", node.id),
                ));
            }
            let round = t.plan_rounds(&node.id, Stage::Research)? + 1;
            t.set_plan_gate(
                &node.id,
                &Gate {
                    stage: Stage::Research,
                    doc_path: None,
                    sha: None,
                    pr_number: None,
                    submitted_by: by.clone(),
                    submitted_at: bus::now(),
                    note: Some(reason.to_string()),
                    children: Vec::new(),
                    decision_id: None,
                    round,
                },
            )?;
            let stage = Some(Stage::Research);
            changed(t, wrote, &node.id, &by, "submitted", stage, Some(reason))?;
            Ok(Step::Done(node.id))
        })
    }
}
