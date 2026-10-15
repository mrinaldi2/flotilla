//! The second rename's move: a pre-rename 0.19 home (`~/.thehermes`, itself
//! moved from `~/.gravity` by the first rename, whose finished log and
//! `meta.home_migrated` it still carries) to `~/.flotilla`.

use super::resume_tests::crash_at;
use super::tests::workspace_path;
use super::*;
use crate::activity::transcript_dir;
use crate::config::Config;

/// `~/.thehermes` as The Hermes 0.19 leaves it on the owner's Mac.
pub(super) struct Hermes {
    _tmp: tempfile::TempDir,
    pub(super) user: PathBuf,
    pub(super) gravity: PathBuf,
    pub(super) plan: Plan,
    pub(super) old_ws: PathBuf,
    pub(super) new_ws: PathBuf,
}

fn meta(db: &Path, key: &str) -> Option<String> {
    let conn = rusqlite::Connection::open(db).expect("conn");
    conn.query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| r.get(0))
        .ok()
}

pub(super) fn hermes_home() -> Hermes {
    let tmp = tempfile::tempdir().expect("tmp");
    let user = tmp.path().join("user");
    let gravity = user.join(".gravity");
    let from = user.join(".thehermes");
    let to = user.join(".flotilla");
    let old_ws = from.join("projects/phd/bots/lead/workspace");
    let new_ws = to.join("projects/phd/bots/lead/workspace");
    std::fs::create_dir_all(&old_ws).expect("workspace");
    std::fs::create_dir_all(from.join("logs")).expect("logs");
    std::fs::create_dir_all(from.join("bin")).expect("bin");
    std::fs::create_dir_all(from.join("secrets")).expect("secrets");
    let config = format!(
        "port = 49777\nclaude_args = ['--settings', '{}/bot-settings.json']\n",
        from.display()
    );
    std::fs::write(from.join("hermesd.toml"), config).expect("config");
    let settings = r#"{"permissions": {"deny": ["Read(~/.thehermes/secrets/**)", "Edit(~/.thehermes/hermesd.toml)"]}}"#;
    std::fs::write(from.join("bot-settings.json"), settings).expect("settings");
    std::fs::write(from.join("hermesd.port"), "49777\n").expect("port");
    std::fs::write(from.join("logs/hermesd.err.log"), "log\n").expect("log");
    std::fs::write(from.join("bin/hermesd"), "old daemon").expect("binary");
    std::fs::write(from.join("secrets/device-d1.sha256"), "hash").expect("device");
    std::fs::write(from.join("secrets/peer-p1.token"), "link").expect("peer");

    crate::db::Db::open(&from.join("bus.sqlite")).expect("db");
    let conn = rusqlite::Connection::open(from.join("bus.sqlite")).expect("conn");
    conn.execute(
        "INSERT INTO project(id, name, created_at) VALUES ('p', 'phd', 'now')",
        [],
    )
    .expect("project");
    conn.execute(
        "INSERT INTO bot(id, project_id, name, instructions, workspace_path, created_at)
         VALUES ('b', 'p', 'lead', 'board', ?1, 'now')",
        [old_ws.to_string_lossy()],
    )
    .expect("bot");
    // The first rename's marks.
    for (key, value) in [
        ("home_migrated", "1".to_string()),
        ("home_migrated_from", gravity.display().to_string()),
    ] {
        conn.execute(
            "INSERT INTO meta(key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = ?2",
            rusqlite::params![key, value],
        )
        .expect("meta");
    }
    drop(conn);
    // The first rename's finished log, and its link.
    let mut first = State::new(&gravity, &from);
    first.steps = Step::ALL.to_vec();
    first.completed_at = Some("2026-10-05T04:31:49Z".into());
    std::fs::write(
        from.join(STATE_FILE),
        serde_json::to_string(&first).expect("json"),
    )
    .expect("first log");
    steps::make_link(&from, &gravity).expect("gravity link");

    let transcripts = transcript_dir(&user, &old_ws);
    std::fs::create_dir_all(transcripts.join("memory")).expect("transcripts");
    std::fs::write(transcripts.join("s1.jsonl"), "{}\n").expect("session");

    Hermes {
        plan: Plan::new(from, to, user.clone()),
        _tmp: tmp,
        user,
        gravity,
        old_ws,
        new_ws,
    }
}

fn config(h: &Hermes) -> Config {
    Config {
        home: h.plan.to.clone(),
        user_home: h.user.clone(),
        ..Config::default()
    }
}

/// Everything under `~/.flotilla`: config renamed and rewritten, paths in
/// the database, devices and links, transcripts, the marks of both moves.
fn assert_moved(h: &Hermes) {
    let (from, to) = (&h.plan.from, &h.plan.to);
    let state = h.plan.state().expect("state").expect("this move's log");
    assert!(state.is_complete(), "{:?}", state.steps);
    let config = std::fs::read_to_string(to.join("flotillad.toml")).expect("config");
    assert!(config.contains(&format!("'{}/bot-settings.json'", to.display())));
    assert!(to.join("flotillad.port").exists());
    assert!(to.join("logs/flotillad.err.log").exists());
    assert!(!to.join("hermesd.toml").exists());
    let settings = std::fs::read_to_string(to.join("bot-settings.json")).expect("settings");
    assert!(
        settings.contains("Read(~/.flotilla/secrets/**)"),
        "{settings}"
    );
    assert!(
        settings.contains("Edit(~/.flotilla/flotillad.toml)"),
        "{settings}"
    );
    assert!(
        settings.contains("Read(~/.thehermes/secrets/**)"),
        "kept: {settings}"
    );
    assert!(to.join("secrets/device-d1.sha256").exists());
    assert!(to.join("secrets/peer-p1.token").exists());
    let db = to.join("bus.sqlite");
    assert_eq!(workspace_path(&db), h.new_ws.to_string_lossy());
    assert!(is_migrated(to));
    assert_eq!(
        meta(&db, "home_migrated_to").as_deref(),
        Some(to.to_string_lossy().as_ref())
    );
    let history = meta(&db, "home_migrations").expect("history");
    assert!(
        history.contains(".gravity") && history.contains(".flotilla"),
        "{history}"
    );
    assert!(transcript_dir(&h.user, &h.new_ws).join("s1.jsonl").exists());
    assert!(!transcript_dir(&h.user, &h.old_ws).exists());
    // The first rename's log is kept beside this one's.
    let first = State::load(&to.join("migrate-home.gravity-to-thehermes.json")).expect("kept");
    assert_eq!(first.to, *from);
    // Both old paths still reach the board.
    assert!(from
        .symlink_metadata()
        .expect("link")
        .file_type()
        .is_symlink());
    assert!(from.join("bus.sqlite").exists());
    assert!(h.gravity.join("bus.sqlite").exists());
}

/// Back to `~/.thehermes` exactly as The Hermes left it.
pub(super) fn assert_back(h: &Hermes) {
    let from = &h.plan.from;
    assert!(from.symlink_metadata().expect("home").is_dir());
    assert!(!h.plan.to.exists());
    assert!(from.join("hermesd.toml").exists());
    let db = from.join("bus.sqlite");
    assert_eq!(workspace_path(&db), h.old_ws.to_string_lossy());
    assert_eq!(meta(&db, "home_migrated").as_deref(), Some("1"));
    assert_eq!(
        meta(&db, "home_migrated_from").as_deref(),
        Some(h.gravity.to_string_lossy().as_ref())
    );
    assert_eq!(meta(&db, "home_migrated_to"), None);
    let first = State::load(&from.join(STATE_FILE)).expect("first log back");
    assert!(first.is_complete());
    assert_eq!(first.from, h.gravity);
    assert!(transcript_dir(&h.user, &h.old_ws).join("s1.jsonl").exists());
}

/// The first rename's finished log and `meta.home_migrated` used to make
/// this move look done before it started.
#[test]
fn the_first_renames_marks_do_not_count_as_this_move() {
    let h = hermes_home();
    let cfg = config(&h);
    let plan = Plan::default_for(&cfg);
    assert_eq!(plan.from, h.plan.from);
    assert!(
        plan.state().expect("state").is_none(),
        "not this move's log"
    );
    assert!(!is_migrated(&h.plan.from));
    assert!(pending_unless(&cfg, false).expect("pending").is_some());
    assert!(!completed(&cfg));
}

#[test]
fn a_hermes_home_moves_and_a_second_run_does_nothing() {
    let h = hermes_home();
    let state = run(&h.plan, &mut Vec::new()).expect("migrate");
    let backup = state.backup.expect("backup");
    assert!(backup.join("bus.sqlite").exists(), "backed up first");
    assert!(backup.join("hermesd.toml").exists());
    assert_moved(&h);
    let cfg = config(&h);
    assert!(completed(&cfg));
    assert!(pending_unless(&cfg, false).expect("pending").is_none());

    let mut out = Vec::new();
    run(&h.plan, &mut out).expect("again");
    assert!(String::from_utf8(out)
        .expect("utf8")
        .contains("already migrated"));
    assert_moved(&h);
}

/// Killed after any step, the next run finishes the move, or a rollback
/// puts The Hermes back, first log and marks included.
#[test]
fn an_interrupted_move_resumes_or_rolls_back() {
    for step in Step::ALL {
        for resume in [true, false] {
            let h = hermes_home();
            crash_at(Some(&format!("{step:?}")));
            let crashed = run(&h.plan, &mut Vec::new());
            crash_at(None);
            assert!(crashed.is_err(), "{step:?}");
            if resume {
                run(&h.plan, &mut Vec::new())
                    .unwrap_or_else(|e| panic!("resume after {step:?}: {e:#}"));
                assert_moved(&h);
            } else {
                rollback(&h.plan, &mut Vec::new())
                    .unwrap_or_else(|e| panic!("rollback after {step:?}: {e:#}"));
                assert_back(&h);
            }
        }
    }
}

#[test]
fn a_finished_move_rolls_back_and_can_run_again() {
    let h = hermes_home();
    run(&h.plan, &mut Vec::new()).expect("migrate");
    // The new daemon ran and stopped, taking its port file with it.
    std::fs::remove_file(h.plan.to.join("flotillad.port")).expect("port file");
    rollback(&h.plan, &mut Vec::new()).expect("rollback");
    assert_back(&h);
    std::fs::write(h.plan.from.join("hermesd.port"), "49777\n").expect("port");
    run(&h.plan, &mut Vec::new()).expect("migrate again");
    assert_moved(&h);
}

/// A machine that never took the first rename moves straight from
/// `~/.gravity`.
#[test]
fn a_gravity_home_moves_straight_to_flotilla() {
    let f = super::tests::fixture();
    let cfg = Config {
        home: f.plan.to.clone(),
        user_home: f.plan.user_home.clone(),
        ..Config::default()
    };
    let plan = pending_unless(&cfg, false)
        .expect("pending")
        .expect("a move");
    assert_eq!(plan.from, f.plan.user_home.join(".gravity"));
    run(&plan, &mut Vec::new()).expect("migrate");
    assert!(plan.to.join("flotillad.toml").exists());
}

/// A daemon whose config puts its home elsewhere (a test, a dev daemon)
/// never waits on the owner's real `~/.thehermes`.
#[test]
fn a_home_elsewhere_leaves_the_default_one_alone() {
    let h = hermes_home();
    let cfg = Config {
        home: h.user.join("elsewhere"),
        user_home: h.user.clone(),
        ..Config::default()
    };
    // `pending` reads the environment too; this test's has no home set.
    if !crate::config::home_is_overridden() {
        assert!(pending(&cfg).expect("pending").is_none());
    }
    assert!(refuse_unmigrated(&cfg).is_ok() || crate::config::home_is_overridden());
}
