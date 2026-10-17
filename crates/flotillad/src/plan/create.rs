//! Turning planning on, and the owner's and the lead's direct changes:
//! starting a goal or an initiative, proposing one, editing a node, and
//! dropping a subtree.

use bus::contract::board as c;

use super::model::{Gate, Kind, NewNode, NodeEdit, ResearchState, Stage};
use super::{changed, disabled, owner_only, refused, Planned, Planner, Step, Wrote};
use crate::db::BoardTx;
use crate::decisions::{invalid, not_found};
use crate::kanban::defaults::key_for;
use crate::kanban::guards::unmet;
use crate::kanban::model::Unmet;
use crate::kanban::presets::{Preset, Start};

/// The parent a new initiative names: a goal of the project still open.
fn goal_parent(t: &BoardTx<'_>, project_id: &str, parent: Option<&str>) -> anyhow::Result<bool> {
    let Some(parent) = parent.filter(|p| !p.is_empty()) else {
        return Ok(false);
    };
    Ok(t.plan_node(parent)?
        .is_some_and(|p| p.project_id == project_id && p.kind == Kind::Goal && !p.stage.is_final()))
}

fn enabled(t: &BoardTx<'_>, project_id: &str) -> anyhow::Result<bool> {
    Ok(t.plan_settings(project_id)?.is_some_and(|s| s.enabled))
}

fn no_parent_goal() -> Step {
    refused(
        "plan.parent",
        "An initiative belongs to a goal of this project that is still open.",
    )
}

impl Planner<'_> {
    /// The owner turning planning on or off. On needs the project's
    /// repository, and its board here (started when the project has none
    /// and no link); off keeps every node. `Some` says why not.
    pub fn enable(&self, project_id: &str, enabled: bool) -> anyhow::Result<Option<Unmet>> {
        if !self.actor.is_owner() {
            return Ok(Some(owner_only()));
        }
        let db = &self.app.db;
        if !db
            .get_project(project_id)?
            .is_some_and(|p| p.deleted_at.is_none())
        {
            return Err(not_found(format!("no project {project_id}")));
        }
        if enabled {
            if db.project_repo(project_id)?.is_none() {
                return Ok(Some(unmet(
                    "plan.no_repo",
                    "Planning needs the project's repository: its documents live there.",
                    Some("Set the project's repository in its settings first."),
                )));
            }
            if db.board_settings(project_id)?.is_none() {
                if !db.project_links(project_id)?.is_empty() {
                    return Ok(Some(unmet(
                        "plan.no_board",
                        "This project is linked with another computer and has no board yet.",
                        Some("Start its board on the computer that should be its home."),
                    )));
                }
                db.ensure_board_from(
                    project_id,
                    &db.daemon_id()?,
                    None,
                    &Start::of(Preset::FIRST),
                )?;
            }
        }
        db.board_tx(|t| t.set_plan_enabled(project_id, enabled))?;
        tell_bots(self.app, project_id, enabled);
        Ok(None)
    }

    /// The owner starting a goal, or an initiative under a goal: the
    /// initiative gets its own board and starts in research.
    pub fn create(&self, r: &c::PlanCreate) -> anyhow::Result<Planned> {
        let kind = Kind::from_wire(r.kind).map_err(|e| invalid(e.to_string()))?;
        let lead = self.lead_named(&r.project_id, r.lead.as_deref())?;
        let title = r.title.trim();
        let by = self.by();
        self.write(|t, wrote| {
            if !self.actor.is_owner() {
                return Ok(Step::Refused(vec![owner_only()]));
            }
            if !enabled(t, &r.project_id)? {
                return Ok(Step::Refused(vec![disabled()]));
            }
            if title.is_empty() {
                return Ok(refused("plan.title", "A node needs a title."));
            }
            let parent = r.parent_id.as_deref().filter(|p| !p.is_empty());
            let (stage, research) = match kind {
                Kind::Goal if parent.is_some() => {
                    return Ok(refused("plan.parent", "A goal has no parent."))
                }
                Kind::Goal => (Stage::Define, None),
                Kind::Initiative if !goal_parent(t, &r.project_id, parent)? => {
                    return Ok(no_parent_goal())
                }
                Kind::Initiative => (Stage::Research, Some(ResearchState::Open)),
                Kind::Epic | Kind::Feature => {
                    return Ok(refused(
                        "plan.kind",
                        "Epics and features come from the owner's approvals: an epic from \
                         its initiative's PRD, a feature from its epic's HLD.",
                    ))
                }
            };
            let board = match kind {
                Kind::Initiative => {
                    let board = t.create_board(
                        &r.project_id,
                        title,
                        &key_for(title),
                        Preset::SimpleReview,
                    )?;
                    wrote.boards.push(board.id.clone());
                    Some(board.id)
                }
                _ => None,
            };
            let node = t.insert_plan_node(
                &NewNode {
                    project_id: &r.project_id,
                    kind,
                    parent_id: parent,
                    title,
                    summary: r.summary.as_deref().unwrap_or_default().trim(),
                    stage,
                    due: r.due.as_deref().map(str::trim).filter(|d| !d.is_empty()),
                    lead: lead.as_deref(),
                    board_id: board.as_deref(),
                    research,
                },
                &by,
            )?;
            created(t, wrote, &node.id, &by, "created", stage)
        })
    }

    /// The lead proposing an initiative under a goal. It waits in
    /// `proposed`, with a gate for the owner, and gets its board once
    /// accepted.
    pub fn propose(&self, r: &c::PlanPropose) -> anyhow::Result<Planned> {
        let project_lead = self.project_lead(&r.project_id)?;
        let title = r.title.trim();
        let by = self.by();
        self.write(|t, wrote| {
            if !self.sees(&r.project_id) || !self.leads(None, project_lead.as_deref()) {
                return Ok(refused(
                    "plan.lead_only",
                    "Only the project's lead or the owner proposes an initiative.",
                ));
            }
            if !enabled(t, &r.project_id)? {
                return Ok(Step::Refused(vec![disabled()]));
            }
            if title.is_empty() {
                return Ok(refused("plan.title", "A node needs a title."));
            }
            if !goal_parent(t, &r.project_id, Some(&r.parent_id))? {
                return Ok(no_parent_goal());
            }
            let summary = r.summary.trim();
            let node = t.insert_plan_node(
                &NewNode {
                    project_id: &r.project_id,
                    kind: Kind::Initiative,
                    parent_id: Some(&r.parent_id),
                    title,
                    summary,
                    stage: Stage::Proposed,
                    due: None,
                    lead: None,
                    board_id: None,
                    research: None,
                },
                &by,
            )?;
            t.set_plan_gate(
                &node.id,
                &Gate {
                    stage: Stage::Proposed,
                    doc_path: None,
                    sha: None,
                    pr_number: None,
                    submitted_by: by.clone(),
                    submitted_at: bus::now(),
                    note: (!summary.is_empty()).then(|| summary.to_string()),
                    children: Vec::new(),
                    decision_id: None,
                    round: 1,
                },
            )?;
            created(t, wrote, &node.id, &by, "proposed", Stage::Proposed)
        })
    }

    /// A node's title, summary, due and lead, by the owner or its lead; a
    /// goal's stage (define → active → met), by the owner.
    pub fn update(&self, r: &c::PlanUpdate) -> anyhow::Result<Planned> {
        let Some(project_id) = self.project_of(&r.id)? else {
            return Ok(self.missing(&r.id));
        };
        let project_lead = self.project_lead(&project_id)?;
        let lead: Option<Option<String>> = match r.lead.as_deref().map(str::trim) {
            Some("") => Some(None),
            Some(name) => Some(self.lead_named(&project_id, Some(name))?),
            None => None,
        };
        let stage = r
            .stage
            .map(Stage::from_wire)
            .transpose()
            .map_err(|e| invalid(e.to_string()))?;
        let by = self.by();
        self.write(|t, wrote| {
            let node = match self.open_node(t, &r.id)? {
                Ok(node) => node,
                Err(unmet) => return Ok(Step::Refused(vec![unmet])),
            };
            if node.version != r.expected_version {
                return Ok(Step::Conflict(Box::new(node)));
            }
            if stage.is_some() && !self.actor.is_owner() {
                return Ok(Step::Refused(vec![owner_only()]));
            }
            if !self.leads(Some(&node), project_lead.as_deref()) {
                return Ok(refused(
                    "plan.lead_only",
                    "Only the node's lead or the owner edits it.",
                ));
            }
            if r.title.as_deref().is_some_and(|t| t.trim().is_empty()) {
                return Ok(refused("plan.title", "A node needs a title."));
            }
            if let Some(stage) = stage.filter(|s| *s != node.stage) {
                let moves = matches!(
                    (node.kind, node.stage, stage),
                    (Kind::Goal, Stage::Define, Stage::Active)
                        | (Kind::Goal, Stage::Active, Stage::Met)
                );
                if !moves {
                    return Ok(refused(
                        "plan.stage",
                        format!(
                            "A {} doesn't move from {} to {} by hand: only a goal moves from \
                             define to active, and from active to met.",
                            node.kind.as_str(),
                            node.stage.as_str(),
                            stage.as_str()
                        ),
                    ));
                }
            }
            let edit = NodeEdit {
                title: r.title.as_deref().map(str::trim),
                summary: r.summary.as_deref(),
                due: r
                    .due
                    .as_deref()
                    .map(|d| Some(d.trim()).filter(|d| !d.is_empty())),
                lead: lead.as_ref().map(|l| l.as_deref()),
            };
            let fields = t.edit_plan_node(&node.id, &edit)?;
            if !fields.is_empty() {
                let note = fields.join(", ");
                changed(t, wrote, &node.id, &by, "edited", None, Some(&note))?;
            }
            if let Some(stage) = stage.filter(|s| *s != node.stage) {
                t.set_plan_stage(&node.id, stage)?;
                changed(t, wrote, &node.id, &by, "stage", Some(stage), None)?;
            }
            Ok(Step::Done(node.id))
        })
    }

    /// The owner stopping a node: it and every node under it not yet
    /// finished are dropped, their gates withdrawn. Their cards stay.
    pub fn drop_node(&self, r: &c::PlanDrop) -> anyhow::Result<Planned> {
        let by = self.by();
        let reason = r.reason.as_deref().map(str::trim).filter(|r| !r.is_empty());
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
            if node.stage == Stage::Dropped {
                return Ok(refused(
                    "plan.stage",
                    format!("{} is dropped already.", node.id),
                ));
            }
            for id in t.plan_subtree(&node.id)? {
                let Some(under) = t.plan_node(&id)? else {
                    continue;
                };
                if id != node.id && under.stage.is_final() {
                    continue;
                }
                t.set_plan_stage(&id, Stage::Dropped)?;
                t.clear_plan_gate(&id)?;
                changed(t, wrote, &id, &by, "dropped", Some(Stage::Dropped), reason)?;
            }
            Ok(Step::Done(node.id))
        })
    }

    /// A bot of the project, by id or name, as a node's lead.
    fn lead_named(&self, project_id: &str, name: Option<&str>) -> anyhow::Result<Option<String>> {
        match name.map(str::trim).filter(|n| !n.is_empty()) {
            Some(name) => crate::kanban::edit::project_bot(self.app, project_id, name)
                .map(Some)
                .map_err(|e| invalid(e.to_string())),
            None => Ok(None),
        }
    }

    /// The answer to a change of a node that doesn't exist.
    pub(super) fn missing(&self, id: &str) -> Planned {
        Planned {
            outcome: super::Outcome::Refused(vec![super::not_found(id)]),
            created: Vec::new(),
        }
    }
}

/// A node just inserted: its first line of history, and its push.
fn created(
    t: &BoardTx<'_>,
    wrote: &mut Wrote,
    id: &str,
    by: &str,
    kind: &str,
    stage: Stage,
) -> anyhow::Result<Step> {
    t.record_plan_event(id, by, kind, Some(stage), None)?;
    wrote.node(id);
    Ok(Step::Done(id.to_string()))
}

/// Planning switched: every bot of the project gets its prompt rewritten
/// (the plan's section in or out) and one note, so a running bot needn't
/// wait for its next start to know.
fn tell_bots(app: &std::sync::Arc<crate::app::AppState>, project_id: &str, enabled: bool) {
    let text = match enabled {
        true => {
            "The owner turned on planning for this project: goals, initiatives, epics and \
                 features now live in the plan (plan_get), each initiative has its own board, \
                 and features and epics are never cards. Read the plan and docs/guidelines/ \
                 before your next card."
        }
        false => {
            "The owner turned off planning for this project: its plan is hidden and your \
                  cards work as before."
        }
    };
    let bots = app.db.list_bots(Some(project_id)).unwrap_or_default();
    for bot in bots
        .iter()
        .filter(|b| !b.is_linked() && b.deleted_at.is_none())
    {
        if let Err(error) = crate::botmgmt::reprovision(app, bot) {
            tracing::warn!(bot_id = %bot.id, %error, "system.md not rewritten for planning");
        }
        let sender = crate::messaging::daemon_sender();
        let dm = crate::messaging::Dm::new(&bot.id, &sender, bus::MessageKind::Note, text);
        if let Err(error) = crate::messaging::send_dm(&app.db, &app.events, dm) {
            tracing::warn!(bot_id = %bot.id, %error, "planning note not delivered");
        }
    }
}
