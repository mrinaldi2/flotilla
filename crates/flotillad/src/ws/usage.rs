//! Usage and budgets (0.24.0, `usage_v1`): reading the counts and the
//! price table, and the owner's budget, Extend today and prices.

use serde_json::{json, Value};

use super::Conn;
use crate::db::OwnerProof;
use crate::usage::{budget, prices, report};

/// The requests this module serves.
pub(super) const KINDS: &[&str] = &[
    "usage_get",
    "usage_prices",
    "set_usage_prices",
    "set_project_budget",
    "budget_extend",
];

/// The ones only the owner's app or a paired device may send.
pub(super) const OWNER_ONLY: &[&str] = &["set_usage_prices", "set_project_budget", "budget_extend"];

impl Conn {
    pub(super) fn usage_request(
        &self,
        kind: &str,
        req_id: &Value,
        req: &Value,
    ) -> anyhow::Result<()> {
        if OWNER_ONLY.contains(&kind)
            && !matches!(
                self.owner_proof(),
                Some(OwnerProof::Device { .. } | OwnerProof::Ticket)
            )
        {
            self.reply_err(
                req_id,
                "forbidden",
                "only the owner's app or a paired device sets this",
            );
            return Ok(());
        }
        match kind {
            "usage_get" => self.usage_get(req_id, req),
            "usage_prices" => self.usage_prices(req_id),
            "set_usage_prices" => self.set_usage_prices(req_id, req),
            "set_project_budget" => self.set_project_budget(req_id, req),
            _ => self.budget_extend(req_id, req),
        }
    }

    fn usage_get(&self, req_id: &Value, req: &Value) -> anyhow::Result<()> {
        let range = req.get("range").and_then(Value::as_str).unwrap_or("today");
        if report::range_days(range).is_none() {
            self.reply_err(req_id, "invalid_request", "range must be today, 7d or 30d");
            return Ok(());
        }
        let project_id = req.get("project_id").and_then(Value::as_str);
        let bot_id = req.get("bot_id").and_then(Value::as_str);
        let mut reply = report::usage(&self.app, range, project_id, bot_id)?;
        reply["req_id"] = req_id.clone();
        self.send(reply);
        Ok(())
    }

    fn usage_prices(&self, req_id: &Value) -> anyhow::Result<()> {
        let table: Vec<Value> = self
            .app
            .db
            .usage_prices()?
            .iter()
            .map(prices::view)
            .collect();
        let as_of = self
            .app
            .db
            .usage_prices_as_of()?
            .map(|t| t.with_timezone(&chrono::Local).to_rfc3339());
        self.send(json!({
            "type": "usage_prices", "req_id": req_id,
            "prices": table, "prices_as_of": as_of
        }));
        Ok(())
    }

    fn set_usage_prices(&self, req_id: &Value, req: &Value) -> anyhow::Result<()> {
        let table = match prices::parse(req.get("prices").unwrap_or(&Value::Null)) {
            Ok(table) => table,
            Err(why) => {
                self.reply_err(req_id, "invalid_request", &why);
                return Ok(());
            }
        };
        self.app.db.set_usage_prices(&table)?;
        self.usage_prices(req_id)
    }

    fn set_project_budget(&self, req_id: &Value, req: &Value) -> anyhow::Result<()> {
        let project_id = Self::str_field(req, "project_id")?;
        let Some(project) = self.app.db.get_live_project(project_id)? else {
            self.reply_err(req_id, "not_found", "project not found or already deleted");
            return Ok(());
        };
        let weekly = match req.get("weekly_tokens") {
            None | Some(Value::Null) => None,
            Some(v) => match v.as_u64().filter(|n| *n > 0 && i64::try_from(*n).is_ok()) {
                Some(n) => Some(n),
                None => {
                    self.reply_err(
                        req_id,
                        "invalid_request",
                        "weekly_tokens must be a whole number above 0, or null",
                    );
                    return Ok(());
                }
            },
        };
        self.app.db.set_project_budget(&project.id, weekly)?;
        self.budget_changed(req_id, project)
    }

    fn budget_extend(&self, req_id: &Value, req: &Value) -> anyhow::Result<()> {
        let project_id = Self::str_field(req, "project_id")?;
        let Some(project) = self.app.db.get_live_project(project_id)? else {
            self.reply_err(req_id, "not_found", "project not found or already deleted");
            return Ok(());
        };
        if self.app.db.project_budget(&project.id)?.is_none() {
            let why = "the project has no budget to extend";
            return Err(crate::decisions::conflict(why.to_string()));
        }
        self.app
            .db
            .set_budget_extended(&project.id, budget::tomorrow())?;
        self.budget_changed(req_id, project)
    }

    /// The held wakes follow the new budget at once, and clients see it.
    fn budget_changed(&self, req_id: &Value, project: bus::Project) -> anyhow::Result<()> {
        budget::refresh(&self.app);
        crate::usage::changed(&self.app, &project.id);
        self.app.events.push(crate::events::Push::ProjectUpdated {
            project: project.clone(),
        });
        self.send(json!({
            "type": "project", "req_id": req_id,
            "project": super::project_view(&self.app, &project)
        }));
        Ok(())
    }
}
