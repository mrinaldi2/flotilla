//! A rollback either finishes or leaves the home as the move left it: one
//! that would stop on a file in its way refuses before it undoes anything,
//! and one that stops anyway is never taken for a finished move.

use super::hop_tests::{assert_back, hermes_home};
use super::resume_tests::crash_at;
use super::tests::workspace_path;
use super::*;

/// The Hermes 0.19, started again through the `~/.thehermes` link (its app
/// was opened after the move), writes `hermesd.toml` back into the moved
/// home. Undoing the config's rename would then stop halfway, with the link
/// gone and the database already pointing at `~/.thehermes`.
#[test]
fn a_rollback_that_cannot_finish_changes_nothing() {
    let h = hermes_home();
    run(&h.plan, &mut Vec::new()).expect("migrate");
    std::fs::remove_file(h.plan.to.join("flotillad.port")).expect("port file");
    let stray = h.plan.to.join("hermesd.toml");
    std::fs::write(&stray, "port = 1\n").expect("stray config");

    let error = rollback(&h.plan, &mut Vec::new()).expect_err("in the way");
    assert!(format!("{error:#}").contains("hermesd.toml"), "{error:#}");
    let link = h.plan.from.symlink_metadata().expect("link kept");
    assert!(link.file_type().is_symlink());
    let db = h.plan.to.join("bus.sqlite");
    assert_eq!(workspace_path(&db), h.new_ws.to_string_lossy());
    assert!(is_migrated(&h.plan.to));
    let state = h.plan.state().expect("state").expect("log");
    assert!(state.is_complete());

    std::fs::remove_file(&stray).expect("cleared");
    rollback(&h.plan, &mut Vec::new()).expect("rollback");
    assert_back(&h);
}

/// Stopped after its first undo, a rollback leaves a log that no longer
/// reads as a finished move: the new daemon refuses the home and a forward
/// run refuses to mark it done, until the rollback is run again.
#[test]
fn a_rollback_stopped_partway_is_not_a_finished_move() {
    let h = hermes_home();
    run(&h.plan, &mut Vec::new()).expect("migrate");
    std::fs::remove_file(h.plan.to.join("flotillad.port")).expect("port file");

    crash_at(Some("undo"));
    let crashed = rollback(&h.plan, &mut Vec::new());
    crash_at(None);
    assert!(crashed.is_err());
    assert!(pending_for(&h.plan).expect("pending").is_some());
    let cfg = crate::config::Config {
        home: h.plan.to.clone(),
        user_home: h.user.clone(),
        ..crate::config::Config::default()
    };
    assert!(!completed(&cfg));
    let error = run(&h.plan, &mut Vec::new()).expect_err("half undone");
    assert!(format!("{error:#}").contains("--rollback"), "{error:#}");

    rollback(&h.plan, &mut Vec::new()).expect("finish the rollback");
    assert_back(&h);
}

/// Killed between linking `~/.thehermes` and logging the link, a run leaves
/// a link the log does not know; the rollback removes it with the move.
#[test]
fn an_unlogged_link_is_removed_with_the_move() {
    let h = hermes_home();
    crash_at(Some("Transcripts"));
    assert!(run(&h.plan, &mut Vec::new()).is_err());
    crash_at(None);
    steps::make_link(&h.plan.to, &h.plan.from).expect("link");
    rollback(&h.plan, &mut Vec::new()).expect("rollback");
    assert_back(&h);
}
