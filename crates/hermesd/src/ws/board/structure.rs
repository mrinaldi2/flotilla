//! The owner's structure requests (0.19.0 K4): boards, columns, column rules
//! and automations, and the ruling on a lead's proposal. `kanban::admin`
//! decides; this maps the wire to it and answers with the board as it is
//! now.

use bus::contract::board::structure_result::Outcome as Wire;
use bus::contract::board::{self as c, board_request::Request};

use super::{refuse, Refusal};
use crate::kanban::admin::{self, By, Outcome, Structure};
use crate::ws::Conn;

impl Conn {
    /// One of the structure requests, or the ruling on a proposal.
    pub(super) fn structure_request(&self, req: Request) -> Result<c::StructureResult, Refusal> {
        let req = match req {
            Request::BoardCreate(r) => Structure::Create(r),
            Request::BoardUpdate(r) => Structure::Board(r),
            Request::ColumnAdd(r) => Structure::ColumnAdd(r),
            Request::ColumnUpdate(r) => Structure::ColumnUpdate(r),
            Request::ColumnRemove(r) => Structure::ColumnRemove(r),
            Request::AutomationSet(r) => Structure::Automation(r),
            Request::BoardProposalRule(r) => return self.proposal_rule(&r),
            _ => return Err(refuse("invalid_request", "not a structure request")),
        };
        self.structure(req)
    }

    fn structure(&self, req: Structure) -> Result<c::StructureResult, Refusal> {
        let project_id = match &req {
            Structure::Create(r) => r.project_id.clone(),
            _ => self
                .app
                .db
                .board(req.board_id())?
                .map(|b| b.project_id)
                .ok_or_else(|| refuse("not_found", format!("no board {}", req.board_id())))?,
        };
        let outcome = admin::structure(&self.app, By::Owner(self.actor()), req).map_err(refusal)?;
        self.outcome(&project_id, outcome)
    }

    fn proposal_rule(&self, r: &c::BoardProposalRule) -> Result<c::StructureResult, Refusal> {
        let project_id = self
            .app
            .db
            .board_proposal(&r.proposal_id)?
            .map(|p| p.project_id)
            .ok_or_else(|| refuse("not_found", format!("no proposal {}", r.proposal_id)))?;
        let outcome = admin::rule(
            &self.app,
            &self.actor(),
            &r.proposal_id,
            r.approve,
            r.note.as_deref(),
        )
        .map_err(refusal)?;
        self.outcome(&project_id, outcome)
    }

    fn outcome(&self, project_id: &str, outcome: Outcome) -> Result<c::StructureResult, Refusal> {
        Ok(match outcome {
            Outcome::Applied { board_id } => c::StructureResult {
                outcome: Some(Wire::Applied(self.snapshot(project_id)?)),
                board_id,
            },
            Outcome::Proposed {
                board_id,
                proposal_id,
            } => c::StructureResult {
                outcome: Some(Wire::ProposalId(proposal_id)),
                board_id,
            },
        })
    }
}

/// A refusal with the decision error's code (`invalid`, `conflict`, …).
fn refusal(e: anyhow::Error) -> Refusal {
    let code = crate::decisions::error_code(&e).unwrap_or("invalid_request");
    refuse(code, e.to_string())
}
