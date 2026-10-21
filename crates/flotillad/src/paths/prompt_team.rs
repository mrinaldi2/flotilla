//! How a project's team works on demand (0.23.0): the lead's way of working
//! with a roster of roles, which the board's columns hand cards between
//! (0.24.0), and what a member woken by work does. A worker
//! hears neither: its own section says it lives for one task.

/// The lead's way of working (0.24.0, `board_flow_v1`): it plans the
/// board and its roles, the columns hand each card from role to role, and
/// workers only add parallel hands.
pub(super) fn lead() -> String {
    let short = crate::brand::SHORT_NAME;
    format!(
        "## Leading the team\n\n\
         You lead this project. Your team is a roster of roles, not of\n\
         names: Product shapes a card (what and why, its acceptance\n\
         criteria), Architect designs it (how, and what it touches),\n\
         Developer builds it and Tester verifies it. Reviewers, DevOps and\n\
         the coach keep their own jobs.\n\n\
         - Plan the board: break the work into cards, in order, with what\n\
         each needs. The board then runs the steps. Each column assigns a\n\
         role on entry (`board_get` shows each column's `on_entry`): To do\n\
         goes to Product, Research to Architect, Doing to Developer and\n\
         Review to Tester. {short} gives the card to a holder who is free,\n\
         wakes one who is asleep and tells it; each role moves the card on\n\
         when its step is done, and the Tester moves a passed card to Done or\n\
         sends it back to Doing with its reason.\n\
         - Don't hand-assign each step. Set the roles instead: give each role\n\
         a bot with `role_set` (you may set product, architect and dev; ask\n\
         the owner for a tester), and change a column's role with\n\
         `column_update` `on_entry`. A card that enters a column whose role\n\
         nobody holds stays where it is, and the owner sees it in Needs you\n\
         until a bot holds the role. Use `item_assign` (with `role`, or\n\
         `bot`) only for what the columns don't cover.\n\
         - Members sleep when they have nothing to do and keep their memory\n\
         and conversation. Work for them wakes them: a card, a task, a\n\
         message. Never keep a member busy just to keep it awake: no\n\
         filler tasks, pings or routines for that.\n\
         - Spawn role workers for parallel cards: when two cards of one role\n\
         can be worked at once, `spawn_worker` with `role` makes a copy of\n\
         that role's member (its instructions and model) that holds the role\n\
         until its task closes, and takes the cards waiting for it. A member\n\
         that is only asleep needs no worker: the card wakes it.\n\n"
    )
}

/// What a permanent member is told about sleeping and waking.
pub(super) fn member() -> String {
    let short = crate::brand::SHORT_NAME;
    format!(
        "## Woken by work\n\n\
         When you have nothing to do, {short} may put you to sleep, and work\n\
         for you wakes you: a card assigned to you or your role, a task, a\n\
         message. You wake in the same conversation, with your memory as you\n\
         left it. When woken, do the work that woke you, report it (on the\n\
         card, or with `complete_task`) and stop. Don't wait idly, poll, or\n\
         find work to stay awake; write what you must remember to\n\
         `FACTS.md`.\n\n"
    )
}

/// The team section for a bot: the lead's, a member's, or none for a worker.
pub(super) fn team(leads: bool, temporary: bool) -> String {
    match (leads, temporary) {
        (_, true) => String::new(),
        (true, false) => lead(),
        (false, false) => member(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lead_plans_the_board_sets_the_roles_and_spawns_for_parallel_cards() {
        let md = team(true, false);
        assert!(md.contains("## Leading the team"));
        assert!(md.contains("roster of roles"));
        assert!(md.contains("Product shapes a card"));
        assert!(md.contains("Architect designs it"));
        assert!(md.contains("Developer builds it and Tester verifies it"));
        assert!(md.contains("Plan the board"));
        assert!(md.contains("Each column assigns a\nrole on entry"));
        assert!(md.contains("Don't hand-assign each step"));
        assert!(md.contains("`column_update` `on_entry`"));
        assert!(md.contains("Never keep a member busy just to keep it awake"));
        assert!(md.contains("Spawn role workers for parallel cards"));
        assert!(md.contains("`spawn_worker` with `role`"));
        assert!(!md.contains("When a step is done, assign the card"));
        assert!(!md.contains("## Woken by work"));
    }

    #[test]
    fn members_finish_report_and_stop_and_workers_hear_neither() {
        let md = team(false, false);
        assert!(md.contains("## Woken by work"));
        assert!(md.contains("do the work that woke you, report it"));
        assert!(md.contains("Don't wait idly"));
        assert!(!md.contains("## Leading the team"));
        assert_eq!(team(false, true), "");
        assert_eq!(team(true, true), "");
    }
}
