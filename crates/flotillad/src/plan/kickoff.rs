//! The lead's next piece of plan work, as a card assigned to it (0.21.0):
//! a bot starts work from an assignment, so a node waiting on the lead
//! gets a card saying what to do, on the board its work goes to. A goal
//! that is active asks for initiatives; an initiative or epic in research
//! for its research (a spike: done with a document, it closes research); a
//! node at a document stage for that document, and a sent-back one for its
//! revision. A document's card closes itself once the node moves on.

use super::model::{Kind, Node, Stage};
use super::Wrote;
use crate::actor::Actor;
use crate::db::{BoardTx, MoveTo, NewItem, NodeCard, Write};
use crate::kanban::model::{ItemType, Priority};

/// A kickoff card: its type, title and what it asks.
struct Card {
    item_type: ItemType,
    title: String,
    description: String,
}

/// A document stage's word and the children its submission proposes.
fn document(stage: Stage) -> Option<(&'static str, &'static str)> {
    match stage {
        Stage::Prd => Some(("PRD", "the epics it creates")),
        Stage::Design => Some(("design", "")),
        Stage::Hld => Some(("HLD", "the features it creates")),
        Stage::Lld => Some(("LLD", "the task cards it creates")),
        _ => None,
    }
}

/// A title's form in a repository path.
fn slug(title: &str) -> String {
    let words: Vec<String> = title
        .to_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect();
    match words.is_empty() {
        true => "node".to_string(),
        false => words.join("-"),
    }
}

/// Where the node's document goes in the repository (`docs/planning.md`).
fn path(t: &BoardTx<'_>, node: &Node, stage: Stage) -> anyhow::Result<String> {
    let mut chain = vec![node.clone()];
    while let Some(parent) = chain.last().and_then(|n| n.parent_id.clone()) {
        match t.plan_node(&parent)? {
            Some(p) => chain.push(p),
            None => break,
        }
    }
    let mut dir = String::from("docs");
    for n in chain.iter().rev() {
        let part = match n.kind {
            Kind::Goal => continue,
            Kind::Initiative => "initiatives",
            Kind::Epic => "epics",
            Kind::Feature => "features",
        };
        dir = format!("{dir}/{part}/{}", slug(&n.title));
    }
    Ok(match stage {
        Stage::Prd => format!("{dir}/prd.md"),
        Stage::Design => format!("{dir}/design/design.md"),
        Stage::Hld => format!("{dir}/hld.md"),
        Stage::Lld => format!("{dir}/lld.md"),
        _ => format!("{dir}/research/"),
    })
}

/// The label every kickoff card carries: a feature's progress leaves them
/// out.
pub const KICKOFF: &str = "kickoff";

const DONE_WHEN: &str = "This card closes by itself once the owner approves.";

/// What the node waits on the lead for, if anything.
fn wanted(t: &BoardTx<'_>, node: &Node) -> anyhow::Result<Option<Card>> {
    if node.gate.is_some() {
        return Ok(None);
    }
    let (id, title) = (&node.id, &node.title);
    let card = match (node.kind, node.stage) {
        (Kind::Goal, Stage::Active) => Card {
            item_type: ItemType::Spike,
            title: format!("Propose initiatives for: {title}"),
            description: format!(
                "The owner's goal {id} “{title}” is active: {}\n\nRead the plan (plan_get) and \
                 docs/guidelines/, then propose the initiatives that serve the goal, each with \
                 plan_propose (parent_id {id}) and a why. The owner accepts or declines each; \
                 an accepted one gets its own board. Move this card on once you have proposed.",
                node.summary
            ),
        },
        (Kind::Initiative | Kind::Epic, Stage::Research)
            if node
                .research
                .as_ref()
                .is_some_and(|r| r.state == super::model::ResearchState::Open) =>
        {
            Card {
                item_type: ItemType::Spike,
                title: format!("Research: {title}"),
                description: format!(
                    "Research {id} “{title}” before its {}: answer what it needs to know \
                     (playtests, telemetry, references, the code), write the findings in the \
                     repository under {}, and link each document to this card (item_link kind \
                     artifact). More spikes for {id} (plan_node_id) split the work. Research \
                     closes once every spike of {id}, this one included, is done with a \
                     document; then write the {} and submit it.",
                    if node.kind == Kind::Initiative {
                        "PRD"
                    } else {
                        "design"
                    },
                    path(t, node, Stage::Research)?,
                    if node.kind == Kind::Initiative {
                        "PRD"
                    } else {
                        "design"
                    },
                ),
            }
        }
        (_, stage) => {
            let Some((doc, children)) = document(stage) else {
                return Ok(None);
            };
            let events = t.plan_events(id)?;
            let rounds = events
                .iter()
                .filter(|e| e.kind == "sent_back" && e.stage == Some(stage))
                .count();
            let revise = events
                .last()
                .filter(|e| e.kind == "sent_back" && e.stage == Some(stage))
                .and_then(|e| e.note.clone());
            let round = match rounds {
                0 | 1 => String::new(),
                n => format!(" ({n})"),
            };
            let proposing = match children {
                "" => String::new(),
                c => format!(", proposing {c} (children)"),
            };
            let at = path(t, node, stage)?;
            match revise {
                Some(note) => Card {
                    item_type: ItemType::Design,
                    title: format!("Revise the {doc}{round}: {title}"),
                    description: format!(
                        "The owner sent the {doc} of {id} “{title}” back: “{note}”\n\nChange \
                         {at} as asked and submit it again with plan_submit (id {id}){proposing}. \
                         {DONE_WHEN}"
                    ),
                },
                None => Card {
                    // A PRD, design, HLD or LLD is a spec: a design ticket.
                    item_type: ItemType::Design,
                    title: format!("Write the {doc}: {title}"),
                    description: format!(
                        "Write the {doc} of {id} “{title}” in the repository at {at}, on a branch, \
                         then submit it with plan_submit (id {id}, doc_path {at}){proposing}. \
                         Follow docs/guidelines/. {DONE_WHEN}"
                    ),
                },
            }
        }
    };
    Ok(Some(card))
}

/// Whether `card` is a kickoff the node no longer waits on: a document's
/// for a stage it left or an earlier round of the one it is at, its
/// research once research is over, its proposals once the goal is.
fn passed(card: &NodeCard, node: &Node, want: Option<&Card>) -> bool {
    let doc = document(node.stage).map(|(d, _)| d);
    let of = |word: &str| {
        card.title.starts_with(&format!("Write the {word}: "))
            || card.title.starts_with(&format!("Revise the {word}"))
    };
    let is_doc = ["PRD", "design", "HLD", "LLD"].iter().any(|w| of(w));
    let current = doc.is_some_and(of);
    let superseded = want.is_some_and(|w| w.title != card.title);
    let research_over = card.title.starts_with("Research: ")
        && node
            .research
            .as_ref()
            .is_none_or(|r| r.state != super::model::ResearchState::Open);
    let proposing_over =
        card.title.starts_with("Propose initiatives for: ") && node.stage != Stage::Active;
    (is_doc && (!current || superseded)) || research_over || proposing_over
}

/// Give each node `wrote` changed the card it waits on, assigned to its
/// lead, and close the document cards of the stages it left. The cards
/// made are pushed and their leads told once the write commits.
pub(super) fn ensure(
    t: &BoardTx<'_>,
    wrote: &mut Wrote,
    ids: &[String],
    by: &Actor<'_>,
) -> anyhow::Result<()> {
    for id in ids {
        let Some(node) = t.plan_node(id)? else {
            continue;
        };
        let cards = t.plan_node_cards(id)?;
        let want = match node.stage.is_final() {
            true => None,
            false => wanted(t, &node)?,
        };
        for card in cards
            .iter()
            .filter(|c| !c.closed && passed(c, &node, want.as_ref()))
        {
            let Some(done) = t.done_column_of(&card.board_id)? else {
                continue;
            };
            let to = MoveTo {
                column: &done,
                note: Some("the plan moved on"),
                wip_override: false,
                first: false,
            };
            if let Write::Done(_) = t.move_item(&card.id, card.version, &to, &Actor::Daemon)? {
                wrote.cards.push(card.id.clone());
            }
        }
        let Some(want) = want else {
            continue;
        };
        // Once made, a kickoff is the lead's to work and close; it is never
        // made twice.
        if cards.iter().any(|c| c.title == want.title) {
            continue;
        }
        let lead = match node.lead.clone() {
            Some(lead) => Some(lead),
            None => t.plan_project_lead(&node.project_id)?,
        };
        let board = match node.kind {
            Kind::Goal => t.default_board_of(&node.project_id)?,
            _ => node.board_id.clone(),
        };
        let (Some(lead), Some(board)) = (lead, board) else {
            continue;
        };
        let item = t.create_item(
            &NewItem {
                project_id: &node.project_id,
                board_id: Some(&board),
                item_type: want.item_type,
                title: &want.title,
                description: &want.description,
                size: None,
                priority: Priority::P2,
                labels: &[KICKOFF.to_string()],
                parent_id: None,
                acceptance_criteria: &[],
                plan_node_id: Some(&node.id),
            },
            by,
        )?;
        if let Write::Done(item) = t.assign_item(&item.id, item.version, Some(&lead), by)? {
            wrote.cards.push(item.id.clone());
            wrote.assigned.push(item.id);
        }
    }
    Ok(())
}
