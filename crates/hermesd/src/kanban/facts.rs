//! Facts other modules report about an item (0.19.0 K3). Kanban never asks
//! what a PR or a release is; a module says what happened and the item's
//! board decides where, if anywhere, the item goes:
//!
//! - a release-gated board moves it the way 0.18 did, by category;
//! - a custom board looks the fact up in its `automations`; with no column
//!   mapped, the fact is kept on the item for the card to show (owner
//!   decision 3).

use super::model::ColumnCategory;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fact {
    PrOpened,
    PrMerged,
    PrClosed,
    PrChangesRequested,
    ReleaseSubmitted,
    /// Taken out of a submitted package: unsubmitted, superseded, left out.
    ReleaseWithdrawn,
    ReleaseApproved,
    ReleaseHeld,
    ReleaseRework,
    ReleaseDeployed,
    ReleaseDeployFailed,
    /// Its revert was merged after the owner left it out of a release.
    ReleaseLeftOutReverted,
}

impl Fact {
    pub const ALL: &'static [Fact] = &[
        Fact::PrOpened,
        Fact::PrMerged,
        Fact::PrClosed,
        Fact::PrChangesRequested,
        Fact::ReleaseSubmitted,
        Fact::ReleaseWithdrawn,
        Fact::ReleaseApproved,
        Fact::ReleaseHeld,
        Fact::ReleaseRework,
        Fact::ReleaseDeployed,
        Fact::ReleaseDeployFailed,
        Fact::ReleaseLeftOutReverted,
    ];

    /// The name a board's `automations` key on.
    pub fn name(self) -> &'static str {
        match self {
            Fact::PrOpened => "pr.opened",
            Fact::PrMerged => "pr.merged",
            Fact::PrClosed => "pr.closed",
            Fact::PrChangesRequested => "pr.changes_requested",
            Fact::ReleaseSubmitted => "release.submitted",
            Fact::ReleaseWithdrawn => "release.withdrawn",
            Fact::ReleaseApproved => "release.approved",
            Fact::ReleaseHeld => "release.held",
            Fact::ReleaseRework => "release.rework",
            Fact::ReleaseDeployed => "release.deployed",
            Fact::ReleaseDeployFailed => "release.deploy_failed",
            Fact::ReleaseLeftOutReverted => "release.left_out_reverted",
        }
    }

    /// What the owner reads in the automations settings and on a card.
    pub fn description(self) -> &'static str {
        match self {
            Fact::PrOpened => "PR opened",
            Fact::PrMerged => "PR merged",
            Fact::PrClosed => "PR closed without merging",
            Fact::PrChangesRequested => "Owner asked for changes on the PR",
            Fact::ReleaseSubmitted => "Submitted in a release package",
            Fact::ReleaseWithdrawn => "Taken out of a release package",
            Fact::ReleaseApproved => "Release approved to ship",
            Fact::ReleaseHeld => "Held by the owner's release verdict",
            Fact::ReleaseRework => "Sent back by the owner's release verdict",
            Fact::ReleaseDeployed => "Deployed",
            Fact::ReleaseDeployFailed => "Deploy failed",
            Fact::ReleaseLeftOutReverted => "Left out of a release and reverted",
        }
    }

    /// A deploy's outcome: deployed everywhere, or failed somewhere.
    pub fn deploy(done: bool) -> Fact {
        if done {
            Fact::ReleaseDeployed
        } else {
            Fact::ReleaseDeployFailed
        }
    }

    pub fn parse(name: &str) -> Option<Fact> {
        Fact::ALL.iter().copied().find(|f| f.name() == name)
    }

    /// Where a release-gated board moves the item: what 0.18 did.
    pub fn release_gated(self) -> ColumnCategory {
        use ColumnCategory as C;
        match self {
            Fact::PrOpened => C::Review,
            Fact::PrMerged => C::Verify,
            Fact::PrClosed | Fact::PrChangesRequested => C::Doing,
            Fact::ReleaseSubmitted => C::Approval,
            Fact::ReleaseWithdrawn | Fact::ReleaseDeployFailed => C::Verify,
            Fact::ReleaseApproved => C::Deploying,
            Fact::ReleaseHeld => C::Ready,
            Fact::ReleaseRework | Fact::ReleaseLeftOutReverted => C::Doing,
            Fact::ReleaseDeployed => C::Done,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_fact_has_a_unique_name_that_parses_back() {
        let mut names: Vec<&str> = Fact::ALL.iter().map(|f| f.name()).collect();
        for fact in Fact::ALL {
            assert_eq!(Fact::parse(fact.name()), Some(*fact));
        }
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), Fact::ALL.len());
    }
}
