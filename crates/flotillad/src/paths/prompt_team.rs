//! How a project's team works on demand (0.23.0): the lead's way of working
//! with a roster of roles, and what a member woken by work does. A worker
//! hears neither: its own section says it lives for one task.

/// The lead's way of working: a card goes to the role that owns its next
/// step, the holder is woken for it, and workers only add parallel hands.
pub(super) fn lead() -> String {
    let short = crate::brand::SHORT_NAME;
    format!(
        "## Leading the team\n\n\
         You lead this project. Your team is a roster of roles, not of\n\
         names: Product shapes a card (what and why, its acceptance\n\
         criteria), Architect designs it (how, and what it touches),\n\
         Developer builds it and Tester verifies it. Reviewers, DevOps and\n\
         the coach keep their own jobs.\n\n\
         - Assign each card to the role that owns its next step:\n\
         `item_assign` with `role` (`product`, `architect`, `dev`, `tester`)\n\
         instead of `bot`. {short} gives it to a holder who is free, wakes\n\
         one who is asleep, and tells whoever got it; `item_get` shows the\n\
         `assignee`. When a step is done, assign the card to the next\n\
         step's role.\n\
         - A role nobody holds is refused (`role.unheld`). Give it to a bot\n\
         with `role_set` (you may set product, architect and dev), or ask\n\
         the owner.\n\
         - Members sleep when they have nothing to do and keep their memory\n\
         and conversation. Work for them wakes them: a card, a task, a\n\
         message. Never keep a member busy just to keep it awake: no\n\
         filler tasks, pings or routines for that.\n\
         - Spawn workers only to add parallel hands, when two cards of one\n\
         role can be worked at once: `spawn_worker` with `role` makes a\n\
         copy of that role's member (its instructions and model) that holds\n\
         the role until its task closes. A member that is only asleep needs\n\
         no worker: assigning the card wakes it.\n\n"
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
    fn the_lead_assigns_to_roles_and_spawns_only_for_parallel_hands() {
        let md = team(true, false);
        assert!(md.contains("## Leading the team"));
        assert!(md.contains("roster of roles"));
        assert!(md.contains("Product shapes a card"));
        assert!(md.contains("Architect designs it"));
        assert!(md.contains("Developer builds it and Tester verifies it"));
        assert!(md.contains("`item_assign` with `role`"));
        assert!(md.contains("`role.unheld`"));
        assert!(md.contains("Never keep a member busy just to keep it awake"));
        assert!(md.contains("Spawn workers only to add parallel hands"));
        assert!(md.contains("`spawn_worker` with `role`"));
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
