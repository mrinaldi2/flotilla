//! The Research column's rule (0.21.0, `start.research`): before a task or a
//! bug starts, its assignee has read the code and written down what it
//! found, in the card's "Code research" section or as a research document
//! linked to it. Spikes and design tickets are research themselves.

use super::super::model::{Item, ItemType, LinkKind, Unmet};
use super::{section_filled, unmet, Context};

/// The template heading the findings go under.
pub(crate) const SECTION: &str = "Code research";

/// Whether the card needs research before it starts.
pub(crate) fn needs_research(item: &Item) -> bool {
    matches!(item.item_type, ItemType::Task | ItemType::Bug)
}

/// A research document: an artifact linked with "research" in its path or
/// label.
fn has_research_doc(ctx: &Context) -> bool {
    ctx.links.iter().any(|l| {
        l.kind == LinkKind::Artifact
            && (l.target.to_lowercase().contains("research")
                || l.label
                    .as_deref()
                    .is_some_and(|label| label.to_lowercase().contains("research")))
    })
}

/// What stops the card from starting, if anything.
pub(crate) fn unmet_research(item: &Item, ctx: &Context) -> Option<Unmet> {
    if !needs_research(item) || section_filled(&item.description, SECTION) || has_research_doc(ctx)
    {
        return None;
    }
    Some(unmet(
        "start.research",
        "No code research is recorded yet.",
        Some(
            "Write what you found in the code under “Code research”, or link a research document.",
        ),
    ))
}
