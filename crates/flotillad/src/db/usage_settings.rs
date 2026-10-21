//! The usage price table and each project's weekly token budget (0.24.0,
//! `usage_v1`).

use chrono::{DateTime, Utc};
use rusqlite::{params, OptionalExtension};

use super::usage::{int, uint};
use super::{parse_ts, ts, Db};

/// USD per million tokens of each kind, for models whose name contains
/// `model` (`*` for any).
#[derive(Debug, Clone, PartialEq)]
pub struct UsagePrice {
    pub model: String,
    pub input: f64,
    pub output: f64,
    pub cache_read: f64,
    pub cache_write: f64,
    /// Seeded by Flotilla, not set by the owner.
    pub estimate: bool,
}

/// A project's weekly budget as stored.
#[derive(Debug, Clone, PartialEq)]
pub struct StoredBudget {
    pub project_id: String,
    pub weekly_tokens: u64,
    pub extended_until: Option<DateTime<Utc>>,
}

impl Db {
    pub fn usage_prices(&self) -> anyhow::Result<Vec<UsagePrice>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(
            "SELECT model, input, output, cache_read, cache_write, estimate
             FROM usage_price ORDER BY model",
        )?;
        let prices = stmt
            .query_map([], |r| {
                Ok(UsagePrice {
                    model: r.get(0)?,
                    input: r.get(1)?,
                    output: r.get(2)?,
                    cache_read: r.get(3)?,
                    cache_write: r.get(4)?,
                    estimate: r.get::<_, i64>(5)? != 0,
                })
            })?
            .collect::<Result<_, _>>()?;
        Ok(prices)
    }

    /// When the price table was last set; the seeded table's date before.
    pub fn usage_prices_as_of(&self) -> anyhow::Result<Option<DateTime<Utc>>> {
        let at: Option<String> =
            self.lock()
                .query_row("SELECT max(updated_at) FROM usage_price", [], |r| r.get(0))?;
        Ok(at.as_deref().map(parse_ts))
    }

    /// The owner's price table, in place of the one before.
    pub fn set_usage_prices(&self, prices: &[UsagePrice]) -> anyhow::Result<()> {
        let mut conn = self.lock();
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM usage_price", [])?;
        let now = ts(Utc::now());
        for p in prices {
            tx.execute(
                "INSERT OR REPLACE INTO usage_price
                     (model, input, output, cache_read, cache_write, estimate, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6)",
                params![p.model, p.input, p.output, p.cache_read, p.cache_write, now],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn project_budget(&self, project_id: &str) -> anyhow::Result<Option<StoredBudget>> {
        let row: Option<(Option<i64>, Option<String>)> = self
            .lock()
            .query_row(
                "SELECT budget_weekly_tokens, budget_extended_until FROM project WHERE id = ?1",
                params![project_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        Ok(row.and_then(|(weekly, until)| {
            Some(StoredBudget {
                project_id: project_id.to_string(),
                weekly_tokens: uint(weekly?),
                extended_until: until.as_deref().map(parse_ts),
            })
        }))
    }

    /// Every live project with a budget.
    pub fn project_budgets(&self) -> anyhow::Result<Vec<StoredBudget>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(
            "SELECT id, budget_weekly_tokens, budget_extended_until FROM project
             WHERE budget_weekly_tokens IS NOT NULL AND deleted_at IS NULL",
        )?;
        let budgets = stmt
            .query_map([], |r| {
                Ok(StoredBudget {
                    project_id: r.get(0)?,
                    weekly_tokens: uint(r.get(1)?),
                    extended_until: r.get::<_, Option<String>>(2)?.as_deref().map(parse_ts),
                })
            })?
            .collect::<Result<_, _>>()?;
        Ok(budgets)
    }

    /// Sets the weekly budget, or removes it with `None`; either ends an
    /// extension.
    pub fn set_project_budget(&self, project_id: &str, weekly: Option<u64>) -> anyhow::Result<()> {
        self.lock().execute(
            "UPDATE project SET budget_weekly_tokens = ?2, budget_extended_until = NULL
             WHERE id = ?1",
            params![project_id, weekly.map(int)],
        )?;
        Ok(())
    }

    pub fn set_budget_extended(
        &self,
        project_id: &str,
        until: DateTime<Utc>,
    ) -> anyhow::Result<()> {
        self.lock().execute(
            "UPDATE project SET budget_extended_until = ?2 WHERE id = ?1",
            params![project_id, ts(until)],
        )?;
        Ok(())
    }
}
