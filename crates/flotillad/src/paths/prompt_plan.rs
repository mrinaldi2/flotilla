//! The prompt's text about card types, the Research column and the plan
//! (0.21.0, #43). Every tool named in backticks is one the daemon serves.

/// Every project: what a card can be, and the research before Doing.
pub(super) const CARD_TYPES: &str = "## Cards and the Research column\n\n\
     A card is a `task` (build something), a `design` ticket (mockups or a\n\
     spec), a `bug` or a `spike` (a timeboxed question that ends in a\n\
     document). Features and epics are not cards: `item_create` refuses\n\
     them (`new.type_retired`).\n\n\
     A task or bug passes the Research column before Doing. There, read the\n\
     code it touches and write what you found under the card's \"Code\n\
     research\" heading (which files, what you learned, the risk), or link a\n\
     research document (`item_link` kind `artifact`). Doing refuses the card\n\
     until you have (`start.research`). Spikes and design tickets are\n\
     research themselves and skip it. When a design ticket is done, link it\n\
     to the task that builds it (`item_link` kind `design`).\n\n";

/// A project whose owner turned planning on.
pub(super) const PLAN: &str = "## The plan\n\n\
     This project is planned: goals hold initiatives, initiatives hold epics,\n\
     epics hold features, and features hold the task cards. Read it with\n\
     `plan_get` and one node with `plan_node_get`. Each initiative has its\n\
     own board; a card on it names its node (`plan_node_id`): a spike or\n\
     design ticket names the initiative or epic it researches, a task the\n\
     feature it builds.\n\n\
     Each node has stages, and the owner approves every document:\n\
     an initiative's PRD, an epic's design then HLD, a feature's LLD. Write\n\
     a document in the project's repository, on a branch, at the path the\n\
     node's documents use (`docs/initiatives/<initiative>/prd.md`,\n\
     `.../epics/<epic>/design/design.md`, `.../hld.md`,\n\
     `.../features/<feature>/lld.md`), then submit it with `plan_submit`\n\
     (the lead), proposing the children it creates: epics with a PRD,\n\
     features with an HLD, task cards with an LLD. Read any of them with\n\
     `plan_doc_read`.\n\n\
     - Research comes first: an initiative or epic starts in research, and\n\
     its document can't be submitted until every spike of it is done with a\n\
     research document attached or linked. Open the spikes on its board\n\
     with `plan_node_id` set. Only the owner skips research; the lead may\n\
     ask with `plan_research_skip` and a reason.\n\
     - Never create a feature or an epic yourself: they are born when the\n\
     owner approves their parent's document. The lead may propose an\n\
     initiative (`plan_propose`); the owner accepts it.\n\
     - A document sent back comes with the owner's note: change it and\n\
     submit it again.\n\
     - The lead's plan work arrives as cards assigned to it, labelled\n\
     `kickoff`: initiatives to propose for an active goal, a node's\n\
     research, a document to write or revise. Each says what to do and\n\
     where; a document's card closes itself once the owner approves.\n\
     - Read the project's guidelines in `docs/guidelines/` before you work,\n\
     and follow them.\n\n";

/// The plan's section for a bot of a project, empty when planning is off.
pub(super) fn plan(enabled: bool) -> &'static str {
    match enabled {
        true => PLAN,
        false => "",
    }
}
