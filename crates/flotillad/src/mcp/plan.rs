//! The plan tools (0.21.0, #43): every member reads the plan and its
//! documents; the lead proposes initiatives, submits a node's document for
//! the owner's approval and asks to skip research. Rulings, creating goals
//! and initiatives, and dropping nodes are the owner's, in the apps. The
//! plan service checks who may act on which node.

use std::sync::Arc;

use bus::contract::board as c;
use serde_json::{json, Value};

use super::board::{bot_actor, out, refused};
use super::board_schema::{decode, shared, tool, Audience, BoardTool};
use crate::app::AppState;
use crate::plan::{docs, Outcome, Planned, Planner};

pub(super) const PLAN_TOOLS: &[BoardTool] = &[
    shared(
        "plan_get",
        "PlanGet",
        Audience::Member,
        "The project's plan: goals, initiatives, epics and features, each with its stage, \
         documents and what waits for the owner. Features and epics live here, never as cards.",
    ),
    tool("plan_node_get", "PlanNodeGet", Audience::Member),
    shared(
        "plan_doc_read",
        "PlanDocRead",
        Audience::Member,
        "Read a file of the project's repository at a commit (the default branch's head when \
         left out), with diff_from for what changed since a commit. offset 0 and limit 0 read \
         the first 512 KiB; `total` says when there is more.",
    ),
    shared(
        "plan_propose",
        "PlanPropose",
        Audience::Lead,
        "Lead: propose an initiative under a goal. It waits for the owner to accept it.",
    ),
    shared(
        "plan_submit",
        "PlanSubmit",
        Audience::Lead,
        "Lead: submit the node's document (PRD, design, HLD or LLD, committed in the repo) for \
         the owner's approval, with the children it proposes: epics with a PRD, features with an \
         HLD, task cards with an LLD. Research must be closed first.",
    ),
    shared(
        "plan_research_skip",
        "PlanResearchSkip",
        Audience::Lead,
        "Lead: ask the owner to skip a node's research, with the reason.",
    ),
];

pub(super) fn handles(name: &str) -> bool {
    PLAN_TOOLS.iter().any(|t| t.name == name)
}

/// A write's answer: the node and what it created, or the refusal.
fn planned(result: anyhow::Result<Planned>) -> anyhow::Result<Value> {
    let planned = result?;
    match planned.outcome {
        Outcome::Done(node) => Ok(json!({
            "node": out(c::PlanNode::from(*node))?,
            "created": planned.created,
        })),
        Outcome::Refused(unmet) => Err(refused(unmet)),
        Outcome::Conflict(node) => Err(anyhow::anyhow!(
            "conflict: {} changed since version you read; it is at version {} now. Read it \
             again with plan_node_get.",
            node.id,
            node.version
        )),
    }
}

/// A document chunk as a bot reads it: text when it is text.
fn document(doc: c::PlanDocContent) -> anyhow::Result<Value> {
    let text = doc.mime.starts_with("text/") || doc.mime == "image/svg+xml";
    let mut value = json!({
        "path": doc.path,
        "sha": doc.sha,
        "offset": doc.offset,
        "total": doc.total,
        "mime": doc.mime,
    });
    if text {
        value["text"] = json!(String::from_utf8_lossy(&doc.data));
    } else {
        value["bytes"] = json!(doc.data.len());
    }
    if let Some(diff) = doc.diff {
        value["diff"] = json!(diff);
    }
    Ok(value)
}

pub(super) fn call(
    app: &Arc<AppState>,
    bot: &bus::Bot,
    name: &str,
    args: &Value,
) -> anyhow::Result<Value> {
    let project = bot.project_id.as_str();
    let planner = Planner {
        app,
        actor: bot_actor(bot),
    };
    match name {
        "plan_get" => {
            let _: c::PlanGet = decode("PlanGet", args, project)?;
            out(c::PlanTree::from(planner.tree(project)?))
        }
        "plan_node_get" => {
            let r: c::PlanNodeGet = decode("PlanNodeGet", args, project)?;
            match planner.detail(&r.id)? {
                Some(detail) => out(c::PlanNodeDetail::from(detail)),
                None => Err(anyhow::anyhow!("not_found: no plan node {}", r.id)),
            }
        }
        "plan_doc_read" => document(docs::read(app, &decode("PlanDocRead", args, project)?)?),
        "plan_propose" => planned(planner.propose(&decode("PlanPropose", args, project)?)),
        "plan_submit" => planned(planner.submit(&decode("PlanSubmit", args, project)?)),
        "plan_research_skip" => {
            planned(planner.research_skip(&decode("PlanResearchSkip", args, project)?))
        }
        other => Err(anyhow::anyhow!("unknown plan tool {other}")),
    }
}
