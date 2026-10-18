//! The plan (0.21.0, #43, plan.proto): a project's goals, initiatives, epics
//! and features, each with its stage and its documents in the project's
//! repository. Board cards belong to a node through `plan_node_id`.
//!
//! The service (`Planner`) answers the owner over WebSocket and the
//! project's bots over MCP (`mcp::plan`): each operation takes the actor
//! and comes to `Done`, `Refused` with what is not met, or `Conflict` with
//! the node as it is. Every change runs in one board transaction with the
//! children, boards and cards it makes, and is pushed once it has
//! committed (`plan_change`).
//!
//! Research closes on facts, read lazily (`research`): a node's spikes all
//! done with a document. So do nodes whose work shipped (`closure`): a
//! feature once its cards are done, then its epic and initiative; a card
//! reaching Done settles them at once (`watch`).

use std::collections::HashSet;
use std::sync::Arc;

use crate::actor::Actor;
use crate::app::AppState;
use crate::db::BoardTx;
use crate::kanban::feed::{card_after_commit, Change, ChangeKind};
use crate::kanban::guards::unmet;
use crate::kanban::model::Unmet;

mod closure;
pub mod contract;
mod create;
pub mod docs;
mod gates;
mod kickoff;
pub mod model;
mod reads;
mod research;
mod rule;
pub mod watch;

use model::{Node, Stage};
pub use reads::{Detail, Tree};

/// What a plan change came to.
#[derive(Debug)]
pub enum Outcome {
    /// The node as it is after the change.
    Done(Box<Node>),
    /// Not allowed now; nothing was written.
    Refused(Vec<Unmet>),
    /// The node changed since the version the caller read.
    Conflict(Box<Node>),
}

#[derive(Debug)]
pub struct Planned {
    pub outcome: Outcome,
    /// Nodes and cards the change created: an approval's children.
    pub created: Vec<String>,
}

/// Who changes the plan, the owner or a bot of the project.
pub struct Planner<'a> {
    pub app: &'a Arc<AppState>,
    pub actor: Actor<'a>,
}

/// What a write did, for its pushes once it commits.
#[derive(Default)]
struct Wrote {
    nodes: Vec<String>,
    /// Nodes the daemon changed rather than the caller: research a spike
    /// closed, recorded on a read.
    by_daemon: Vec<String>,
    /// Kickoff cards assigned to a lead, whose leads are told once the
    /// write commits (`kickoff`).
    assigned: Vec<String>,
    /// Nodes to check for a kickoff besides those changed: every open node
    /// of a project whose plan was read.
    look: Vec<String>,
    cards: Vec<String>,
    boards: Vec<String>,
    created: Vec<String>,
}

impl Wrote {
    fn node(&mut self, id: &str) {
        if !self.nodes.iter().any(|n| n == id) {
            self.nodes.push(id.to_string());
        }
    }
}

/// How a write's transaction ended.
enum Step {
    Done(String),
    Refused(Vec<Unmet>),
    Conflict(Box<Node>),
}

fn refused(code: &str, text: impl Into<String>) -> Step {
    Step::Refused(vec![unmet(code, text, None)])
}

fn not_found(id: &str) -> Unmet {
    unmet(
        "plan.not_found",
        format!("There is no plan node {id}."),
        None,
    )
}

fn disabled() -> Unmet {
    unmet(
        "plan.disabled",
        "Planning is off for this project.",
        Some("The owner turns it on in the project's plan."),
    )
}

fn owner_only() -> Unmet {
    unmet("plan.owner_only", "Only the owner can do this.", None)
}

impl Planner<'_> {
    fn by(&self) -> String {
        self.actor.as_stored()
    }

    /// A bot reaches only its own project's plan.
    fn sees(&self, project_id: &str) -> bool {
        self.actor.project_id().is_none_or(|p| p == project_id)
    }

    /// The node `id` of a project with planning on, as this caller may see
    /// it, or why not.
    fn open_node(&self, t: &BoardTx<'_>, id: &str) -> anyhow::Result<Result<Node, Unmet>> {
        let Some(node) = t.plan_node(id)?.filter(|n| self.sees(&n.project_id)) else {
            return Ok(Err(not_found(id)));
        };
        if !t
            .plan_settings(&node.project_id)?
            .is_some_and(|s| s.enabled)
        {
            return Ok(Err(disabled()));
        }
        Ok(Ok(node))
    }

    /// Whether this caller leads `node`: the owner, the node's lead, or the
    /// project's lead (`project_lead`, read before the transaction).
    fn leads(&self, node: Option<&Node>, project_lead: Option<&str>) -> bool {
        match self.actor.bot_id() {
            None => self.actor.is_owner(),
            Some(bot) => {
                node.and_then(|n| n.lead.as_deref()) == Some(bot) || project_lead == Some(bot)
            }
        }
    }

    fn project_lead(&self, project_id: &str) -> anyhow::Result<Option<String>> {
        crate::kanban::drafts::lead_of(self.app, project_id)
    }

    /// The project a node is in, read before a write's transaction.
    fn project_of(&self, id: &str) -> anyhow::Result<Option<String>> {
        self.app
            .db
            .board_read(|t| Ok(t.plan_node(id)?.map(|n| n.project_id)))
    }

    /// Run `work` in one board transaction holding the feed, then push what
    /// it changed.
    fn write(
        &self,
        work: impl FnOnce(&BoardTx<'_>, &mut Wrote) -> anyhow::Result<Step>,
    ) -> anyhow::Result<Planned> {
        let (step, created) = self.run(work)?;
        let outcome = match step {
            Step::Done(id) => Outcome::Done(Box::new(
                self.app
                    .db
                    .board_read(|t| t.plan_node(&id))?
                    .ok_or_else(|| anyhow::anyhow!("plan node {id} is gone"))?,
            )),
            Step::Refused(unmet) => Outcome::Refused(unmet),
            Step::Conflict(node) => Outcome::Conflict(node),
        };
        Ok(Planned { outcome, created })
    }

    /// Run `work` in one board transaction holding the feed; once it has
    /// committed, push the boards and cards it made and the nodes it
    /// changed. Returns what `work` did and what it created.
    fn run<T>(
        &self,
        work: impl FnOnce(&BoardTx<'_>, &mut Wrote) -> anyhow::Result<T>,
    ) -> anyhow::Result<(T, Vec<String>)> {
        let mut feed = self.app.board.writer();
        let mut wrote = Wrote::default();
        // A kickoff's card is made as the caller's; the daemon's own
        // changes (research a read closed) are told to the lead as the
        // owner's, since a notice from the daemon tells nobody.
        let by = match self.actor {
            Actor::Daemon => Actor::User,
            other => other,
        };
        let out = self.app.db.board_tx(|t| {
            let out = work(t, &mut wrote)?;
            let mut ids = wrote.nodes.clone();
            ids.extend(
                wrote
                    .look
                    .iter()
                    .filter(|id| !wrote.nodes.contains(id))
                    .cloned(),
            );
            kickoff::ensure(t, &mut wrote, &ids, &by)?;
            Ok(out)
        })?;
        let db = &self.app.db;
        for id in &wrote.assigned {
            if let Ok(Some(item)) = db.get_item(id) {
                crate::kanban::notices::tell_reassigned(self.app, None, &item, &by);
            }
        }
        for board in &wrote.boards {
            let Some(project_id) = db.board(board)?.map(|b| b.project_id) else {
                continue;
            };
            let change = Change {
                project_id: &project_id,
                kind: ChangeKind::SettingsChanged,
                item_id: "",
                card: None,
                from_column: None,
            };
            feed.publish_on(Some(board), change);
        }
        for card in &wrote.cards {
            let Some(project_id) = db.board_read(|t| t.item_project(card))? else {
                continue;
            };
            feed.publish(Change {
                project_id: &project_id,
                kind: ChangeKind::ItemUpserted,
                item_id: card,
                card: card_after_commit(db, card),
                from_column: None,
            });
        }
        for id in &wrote.nodes {
            match db.board_read(|t| t.plan_node(id)) {
                Ok(Some(node)) => {
                    let actor = match wrote.by_daemon.contains(id) {
                        true => "daemon".to_string(),
                        false => self.by(),
                    };
                    feed.publish_plan(node, Some(actor));
                }
                Ok(None) => {}
                Err(e) => tracing::warn!(node = %id, error = %e, "plan node read after commit"),
            }
        }
        Ok((out, wrote.created))
    }
}

/// Bump the node's version, record its event, and mark it for its push.
fn changed(
    t: &BoardTx<'_>,
    wrote: &mut Wrote,
    id: &str,
    by: &str,
    kind: &str,
    stage: Option<Stage>,
    note: Option<&str>,
) -> anyhow::Result<()> {
    // One version per write, however many lines of history it adds.
    if !wrote.nodes.iter().any(|n| n == id) {
        t.claim_plan_node(id, None)?;
    }
    t.record_plan_event(id, by, kind, stage, note)?;
    wrote.node(id);
    if by == "daemon" && !wrote.by_daemon.iter().any(|n| n == id) {
        wrote.by_daemon.push(id.to_string());
    }
    Ok(())
}

/// Why a card can't belong to plan node `node_id`: it isn't a node of
/// `project_id`.
pub fn unknown_node(
    app: &AppState,
    project_id: &str,
    node_id: &str,
) -> anyhow::Result<Option<Unmet>> {
    let found = app.db.board_read(|t| t.plan_node(node_id))?;
    Ok(match found {
        Some(node) if node.project_id == project_id => None,
        _ => Some(not_found(node_id)),
    })
}

/// The nodes `ids` and every node under them, for an `ItemQuery`; `None`
/// when it names none.
pub fn subtree_ids(app: &AppState, ids: &[String]) -> anyhow::Result<Option<HashSet<String>>> {
    if ids.is_empty() {
        return Ok(None);
    }
    app.db.board_read(|t| {
        let mut all = HashSet::new();
        for id in ids {
            all.extend(t.plan_subtree(id)?);
        }
        Ok(Some(all))
    })
}
