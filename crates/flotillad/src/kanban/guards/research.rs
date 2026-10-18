//! The Research column's rule (0.21.0, `start.research`): before a task or a
//! bug starts, its assignee has read the code and written down what it
//! found, in the card's "Code research" section or as a research document
//! attached or linked to it. Spikes and design tickets are research
//! themselves.

use super::super::model::{Item, ItemType, LinkKind, Unmet};
use super::{section_filled, unmet, Context};

/// The template heading the findings go under.
pub(crate) const SECTION: &str = "Code research";

/// Whether the card needs research before it starts.
pub(crate) fn needs_research(item: &Item) -> bool {
    matches!(item.item_type, ItemType::Task | ItemType::Bug)
}

/// A research document: a file attached, or an artifact linked, with
/// "research" in its name, path or label.
fn has_research_doc(ctx: &Context) -> bool {
    let research = |s: &str| s.to_lowercase().contains("research");
    ctx.attachments.iter().any(|a| research(a))
        || ctx.links.iter().any(|l| {
            l.kind == LinkKind::Artifact
                && (research(&l.target) || l.label.as_deref().is_some_and(research))
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
            "Write what you found in the code under “Code research”, or attach a research \
             document (item_attach).",
        ),
    ))
}

/// A card's type on the wire, before it is mapped: features and epics live
/// in the plan and chores became tasks (0.21.0), so those are refused with a
/// fix rather than as an unknown value.
pub fn check_type(wire: i32) -> Vec<Unmet> {
    use bus::contract::board::ItemType as Wire;
    let retired = match Wire::try_from(wire) {
        Ok(Wire::Epic | Wire::Feature) => {
            "Features and epics are created in the plan, not as cards."
        }
        Ok(Wire::Chore) => "Chores are tasks now.",
        _ => return Vec::new(),
    };
    vec![unmet(
        "new.type_retired",
        retired,
        Some("Use task, design, bug or spike; propose a feature through plan_propose."),
    )]
}
