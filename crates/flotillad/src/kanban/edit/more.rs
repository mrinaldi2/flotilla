//! The item edits that need a live item: blocking, assigning, ranking and
//! acceptance checks.

use std::cell::Cell;

use bus::contract::board as c;

use super::{project_bot, Edited, Editor};
use crate::db::Write;
use crate::kanban::guards::{self, Who};
use crate::kanban::model::Item;
use crate::releases::post_install;

impl Editor<'_> {
    pub fn block(&self, req: &c::ItemBlock) -> anyhow::Result<Edited> {
        if let Some(by) = &req.by {
            self.own_item(by)?;
        }
        let reason = req.reason.trim();
        let guard = |item: &Item, who: &Who| guards::check_block(item, who, true, Some(reason));
        self.guarded(&req.id, req.expected_version, false, guard, |t| {
            t.block_item(
                &req.id,
                req.expected_version,
                Some((req.by.as_deref(), reason)),
                &self.actor,
            )
        })
    }

    pub fn unblock(&self, req: &c::ItemUnblock) -> anyhow::Result<Edited> {
        let guard = |item: &Item, who: &Who| guards::check_block(item, who, false, None);
        self.guarded(&req.id, req.expected_version, false, guard, |t| {
            t.block_item(&req.id, req.expected_version, None, &self.actor)
        })
    }

    pub fn assign(&self, req: &c::ItemAssign) -> anyhow::Result<Edited> {
        let assignee = req
            .bot
            .as_deref()
            .map(|b| project_bot(self.app, self.project_id, b))
            .transpose()?;
        let guard = |_: &Item, who: &Who| guards::check_lead(who);
        self.guarded(&req.id, req.expected_version, false, guard, |t| {
            t.assign_item(
                &req.id,
                req.expected_version,
                assignee.as_deref(),
                &self.actor,
            )
        })
    }

    pub fn rank(&self, req: &c::ItemRank) -> anyhow::Result<Edited> {
        for other in [&req.after, &req.before].into_iter().flatten() {
            self.own_item(other)?;
        }
        let guard = |_: &Item, who: &Who| guards::check_lead(who);
        self.guarded(&req.id, req.expected_version, false, guard, |t| {
            let (after, before) = (req.after.as_deref(), req.before.as_deref());
            t.rank_item(&req.id, req.expected_version, after, before, &self.actor)
        })
    }

    /// Tick or fail one acceptance criterion. The lead's evidence goes on
    /// the item as a comment and into its open packages' history, in the
    /// same write (ARCH-R53 S2). `sender` is who tells the release once a
    /// post-install criterion is proven.
    pub fn check_ac(&self, req: &c::ItemCheckAc, sender: &bus::Sender) -> anyhow::Result<Edited> {
        let passed = c::VerificationResult::try_from(req.result) == Ok(c::VerificationResult::Pass);
        let evidence = req.evidence.as_deref().map(str::trim);
        let lead_check = Cell::new(false);
        let proved_post_install = Cell::new(false);
        let guard = |item: &Item, who: &Who| {
            lead_check.set(guards::needs_evidence(item, who));
            guards::check_ac(item, who, evidence)
        };
        let edited = self.guarded(&req.id, req.expected_version, false, guard, |t| {
            let write = t.check_ac(
                &req.id,
                req.expected_version,
                req.index,
                passed,
                req.machine.as_deref(),
                &self.actor,
            )?;
            if let (true, Some(evidence)) = (lead_check.get(), evidence) {
                let verb = if passed { "ticked" } else { "marked failed" };
                let body = format!(
                    "Acceptance criterion {} {verb} by the lead. Evidence: {evidence}",
                    req.index + 1
                );
                t.add_item_comment(&req.id, &body, None, &self.actor)?;
                let by = match &self.actor {
                    crate::actor::Actor::Bot { id, .. } => *id,
                    _ => "owner",
                };
                post_install::record_lead_tick(t, &req.id, req.index, passed, evidence, by)?;
            }
            if let Write::Done(item) = &write {
                proved_post_install.set(
                    passed
                        && item
                            .acceptance_criteria
                            .iter()
                            .any(|a| a.idx == req.index && a.post_install),
                );
            }
            Ok(write)
        })?;
        if proved_post_install.get() {
            post_install::tell_when_proven(self.app, sender, &req.id)?;
        }
        Ok(edited)
    }

    pub fn flag_ac(&self, req: &c::ItemFlagAc) -> anyhow::Result<Edited> {
        let guard = |item: &Item, who: &Who| guards::flag_ac(item, who, req.post_install);
        self.guarded(&req.id, req.expected_version, false, guard, |t| {
            t.flag_ac(
                &req.id,
                req.expected_version,
                req.index,
                req.post_install,
                &self.actor,
            )
        })
    }
}
