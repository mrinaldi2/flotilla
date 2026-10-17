//! The plan's requests over WebSocket (0.21.0, #43, `plan_v1`), served by
//! the plan service as the owner. A project whose board lives on a peer has
//! its plan there too: it is refused here.

use bus::contract::board::{board_request::Request, board_response::Response};

use super::item_edits::refusal;
use super::{refuse, Refusal};
use crate::plan::{docs, Planner};
use crate::ws::Conn;

/// A refusal's code, from the plan service's (a fixed set).
fn code(text: &str) -> &'static str {
    match text {
        "plan.no_repo" => "plan.no_repo",
        "plan.no_board" => "plan.no_board",
        "plan.owner_only" => "forbidden",
        _ => "invalid_request",
    }
}

impl Conn {
    pub(super) fn plan_request(&self, request: Request) -> Result<Response, Refusal> {
        let project = match &request {
            Request::PlanGet(r) => Some(r.project_id.as_str()),
            Request::PlanEnable(r) => Some(r.project_id.as_str()),
            Request::PlanCreate(r) => Some(r.project_id.as_str()),
            Request::PlanPropose(r) => Some(r.project_id.as_str()),
            Request::PlanDocRead(r) => Some(r.project_id.as_str()),
            _ => None,
        };
        if let Some(home) = project.and_then(|p| self.app.board_mirror.home_peer(p)) {
            let name = crate::peer::board::home_name(&self.app, &home);
            return Err(refuse(
                "no_board",
                format!("This project's plan is kept on {name}; open it there."),
            ));
        }
        let planner = Planner {
            app: &self.app,
            actor: self.actor(),
        };
        let planned = |p: anyhow::Result<crate::plan::Planned>| -> Result<Response, Refusal> {
            Ok(Response::Planned(p.map_err(refusal)?.into()))
        };
        match request {
            Request::PlanGet(r) => Ok(Response::Plan(
                planner.tree(&r.project_id).map_err(refusal)?.into(),
            )),
            Request::PlanNodeGet(r) => match planner.detail(&r.id).map_err(refusal)? {
                Some(detail) => Ok(Response::PlanNode(detail.into())),
                None => Err(refuse("not_found", format!("no plan node {}", r.id))),
            },
            Request::PlanEnable(r) => {
                let refused = planner.enable(&r.project_id, r.enabled).map_err(refusal)?;
                if let Some(unmet) = refused {
                    return Err(refuse(code(&unmet.code), unmet.text));
                }
                Ok(Response::Plan(
                    planner.tree(&r.project_id).map_err(refusal)?.into(),
                ))
            }
            Request::PlanCreate(r) => planned(planner.create(&r)),
            Request::PlanPropose(r) => planned(planner.propose(&r)),
            Request::PlanUpdate(r) => planned(planner.update(&r)),
            Request::PlanSubmit(r) => planned(planner.submit(&r)),
            Request::PlanRule(r) => planned(planner.rule(&r)),
            Request::PlanResearchSkip(r) => planned(planner.research_skip(&r)),
            Request::PlanDrop(r) => planned(planner.drop_node(&r)),
            Request::PlanDocRead(r) => Ok(Response::PlanDoc(
                docs::read(&self.app, &r).map_err(refusal)?,
            )),
            _ => Err(refuse("invalid_request", "not a plan request")),
        }
    }
}
