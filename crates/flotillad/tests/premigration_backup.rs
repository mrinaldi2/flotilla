//! The daemon backs up an older database before it migrates it, and refuses
//! to start, migrating nothing, when the backup can't be taken (0.19.0 plan,
//! "Data migration"). These run the real binary, so the order in `main` is
//! what is tested.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use bus::schema::MIGRATIONS;
use flotillad::db::{schema_version, Db};

struct Home {
    _dirs: (tempfile::TempDir, tempfile::TempDir),
    home: PathBuf,
    config: PathBuf,
}

/// A home whose database, with one project, is one migration behind.
fn older_home() -> Home {
    let home_dir = tempfile::tempdir().expect("home");
    let config_dir = tempfile::tempdir().expect("config dir");
    let home = home_dir.path().to_path_buf();
    let db = Db::open(&home.join("bus.sqlite")).expect("db");
    db.create_project("acme", "acme").expect("project");
    drop(db);
    rusqlite::Connection::open(home.join("bus.sqlite"))
        .expect("raw db")
        .execute(
            "UPDATE meta SET value = ?1 WHERE key = 'schema_version'",
            [(MIGRATIONS.len() - 1).to_string()],
        )
        .expect("rewind");
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .and_then(|l| l.local_addr())
        .expect("free port")
        .port();
    let config = config_dir.path().join("flotillad.toml");
    std::fs::write(
        &config,
        format!(
            "home = {:?}\nport = {port}\nruntime = \"double\"\n",
            home.display().to_string()
        ),
    )
    .expect("config");
    Home {
        _dirs: (home_dir, config_dir),
        home,
        config,
    }
}

fn start(home: &Home) -> Child {
    Command::new(env!("CARGO_BIN_EXE_flotillad"))
        .arg("--config")
        .arg(&home.config)
        .arg("--scratch")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start flotillad")
}

fn version(home: &Path) -> usize {
    let conn = rusqlite::Connection::open(home.join("bus.sqlite")).expect("raw db");
    schema_version(&conn).expect("schema version")
}

fn pre_migration_backups(home: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(home.join("backups"))
        .map(|entries| {
            entries
                .map(|e| e.expect("entry").path())
                .filter(|p| {
                    p.file_name()
                        .is_some_and(|n| n.to_string_lossy().starts_with("pre-migration-"))
                })
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn a_start_on_an_older_database_backs_it_up_then_migrates() {
    let home = older_home();
    let from = MIGRATIONS.len() - 1;
    let mut daemon = start(&home);
    let deadline = Instant::now() + Duration::from_secs(60);
    while version(&home.home) != MIGRATIONS.len() {
        if let Some(status) = daemon.try_wait().expect("poll flotillad") {
            panic!("flotillad exited with {status}");
        }
        assert!(Instant::now() < deadline, "flotillad never migrated");
        std::thread::sleep(Duration::from_millis(100));
    }
    daemon.kill().expect("stop flotillad");
    daemon.wait().expect("reap flotillad");

    let backups = pre_migration_backups(&home.home);
    assert_eq!(backups.len(), 1, "{backups:?}");
    let name = backups[0]
        .file_name()
        .expect("name")
        .to_string_lossy()
        .to_string();
    assert!(
        name.ends_with(&format!("-schema-{from}-to-{}", MIGRATIONS.len())),
        "{name}"
    );
    assert_eq!(version(&backups[0]), from);
}

#[test]
fn a_start_whose_backup_fails_migrates_nothing() {
    let home = older_home();
    let from = MIGRATIONS.len() - 1;
    std::fs::write(home.home.join("backups"), "not a folder").expect("block backups");
    let mut daemon = start(&home);
    let deadline = Instant::now() + Duration::from_secs(60);
    let status = loop {
        if let Some(status) = daemon.try_wait().expect("poll flotillad") {
            break status;
        }
        if Instant::now() > deadline {
            let _ = daemon.kill();
            panic!("flotillad kept running without its backup");
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    let mut stderr = String::new();
    std::io::Read::read_to_string(&mut daemon.stderr.take().expect("stderr"), &mut stderr)
        .expect("read stderr");
    assert!(!status.success(), "{stderr}");
    assert!(stderr.contains("refusing to start"), "{stderr}");
    assert!(stderr.contains("Nothing was migrated"), "{stderr}");
    assert_eq!(version(&home.home), from);
}
