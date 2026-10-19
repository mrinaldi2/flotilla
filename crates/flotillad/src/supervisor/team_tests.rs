use super::*;

fn card(id: &str, priority: &str, urgent: bool, age_days: i64) -> WakeRequest {
    WakeRequest {
        reason: WakeReason::Card,
        card: Some(WakeCard {
            item_id: id.to_string(),
            priority: priority.to_string(),
            urgent,
            created_at: Some(Utc::now() - chrono::Duration::days(age_days)),
        }),
    }
}

fn order(team: &Team) -> Vec<String> {
    team.queue().into_iter().map(|w| w.bot_id).collect()
}

/// One slot, taken: everything after queues.
fn full() -> Team {
    let mut team = Team::default();
    let woke = team.wake(
        "running",
        "p",
        WakeRequest::new(WakeReason::Message),
        1,
        Utc::now(),
    );
    assert_eq!(woke, Woke::Started);
    team
}

#[test]
fn a_free_slot_starts_at_once_and_a_full_one_queues() {
    let mut team = full();
    let woke = team.wake(
        "b",
        "p",
        WakeRequest::new(WakeReason::Message),
        1,
        Utc::now(),
    );
    assert_eq!(woke, Woke::Queued);
    assert_eq!(team.running(), 1);
    let again = team.wake(
        "running",
        "p",
        WakeRequest::new(WakeReason::Owner),
        1,
        Utc::now(),
    );
    assert_eq!(again, Woke::Running);
}

#[test]
fn the_owner_then_the_lead_then_card_work_then_the_rest() {
    let mut team = full();
    let now = Utc::now();
    team.wake("msg", "p", WakeRequest::new(WakeReason::Routine), 1, now);
    team.wake("card", "p", card("H-1", "P3", false, 1), 1, now);
    team.wake("lead", "p", WakeRequest::new(WakeReason::Lead), 1, now);
    team.wake("owner", "p", WakeRequest::new(WakeReason::Owner), 1, now);
    assert_eq!(order(&team), ["owner", "lead", "card", "msg"]);
}

#[test]
fn later_card_work_waits_behind_work_already_queued() {
    let mut team = full();
    team.wake("p2", "p", card("H-1", "P2", false, 1), 1, Utc::now());
    team.wake("p0", "p", card("H-2", "P0", false, 9), 1, Utc::now());
    assert_eq!(order(&team), ["p2", "p0"]);
}

#[test]
fn an_urgent_bug_goes_to_the_front_of_card_work_only() {
    let mut team = full();
    team.wake(
        "lead",
        "p",
        WakeRequest::new(WakeReason::Lead),
        1,
        Utc::now(),
    );
    team.wake("p1", "p", card("H-1", "P1", false, 1), 1, Utc::now());
    team.wake("bug", "p", card("H-2", "P0", true, 0), 1, Utc::now());
    assert_eq!(order(&team), ["lead", "bug", "p1"]);
    let head = &team.queue()[1];
    assert!(head.urgent);
    assert_eq!(head.priority.as_deref(), Some("P0"));
}

#[test]
fn work_queued_together_keeps_the_boards_order() {
    let mut team = full();
    let batch = vec![
        (
            "young".to_string(),
            "p".to_string(),
            card("H-3", "P1", false, 1),
        ),
        (
            "low".to_string(),
            "p".to_string(),
            card("H-1", "P3", false, 9),
        ),
        (
            "old".to_string(),
            "p".to_string(),
            card("H-2", "P1", false, 5),
        ),
    ];
    team.wake_all(batch, 1, Utc::now());
    assert_eq!(order(&team), ["old", "young", "low"]);
}

#[test]
fn freeing_a_slot_starts_the_head() {
    let mut team = full();
    team.wake("card", "p", card("H-1", "P2", false, 1), 1, Utc::now());
    team.wake(
        "owner",
        "p",
        WakeRequest::new(WakeReason::Owner),
        1,
        Utc::now(),
    );
    assert!(team.grant(1).is_empty());
    team.release("running");
    let started: Vec<String> = team.grant(1).into_iter().map(|(id, _)| id).collect();
    assert_eq!(started, ["owner"]);
    assert!(team.holds("owner"));
    assert_eq!(order(&team), ["card"]);
}

#[test]
fn a_queued_bot_moves_up_only_for_the_owner_or_an_urgent_bug() {
    let mut team = full();
    team.wake(
        "a",
        "p",
        WakeRequest::new(WakeReason::Message),
        1,
        Utc::now(),
    );
    team.wake("b", "p", card("H-1", "P2", false, 1), 1, Utc::now());
    // A later card for `a` doesn't move it ahead of `b`.
    team.wake("a", "p", card("H-2", "P1", false, 1), 1, Utc::now());
    assert_eq!(order(&team), ["b", "a"]);
    team.wake("a", "p", WakeRequest::new(WakeReason::Owner), 1, Utc::now());
    assert_eq!(order(&team), ["a", "b"]);
}

#[test]
fn a_new_wake_waits_behind_the_queue_even_with_a_slot_free() {
    let mut team = full();
    team.wake(
        "a",
        "p",
        WakeRequest::new(WakeReason::Message),
        1,
        Utc::now(),
    );
    team.release("running");
    // The slot is the queue's head's, at the next grant.
    let woke = team.wake("b", "p", WakeRequest::new(WakeReason::Owner), 2, Utc::now());
    assert_eq!(woke, Woke::Queued);
    let started: Vec<String> = team.grant(2).into_iter().map(|(id, _)| id).collect();
    assert_eq!(started, ["b", "a"]);
}
