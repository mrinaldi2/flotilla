//! The pre-migration backup: one consistent backup of an older schema, none
//! for a fresh or current database, no migration when it fails, and only the
//! newest few kept.

use std::path::{Path, PathBuf};

use bus::schema::MIGRATIONS;
use serde_json::{json, Value};

use super::*;
use crate::backup::{backup, backups_dir, open_unmigrated, restore};
use crate::config::Config;
use crate::db::{schema_version, Db};

fn home() -> (tempfile::TempDir, Config) {
    let dir = tempfile::tempdir().unwrap();
    let cfg = Config {
        home: dir.path().to_path_buf(),
        ..Config::default()
    };
    (dir, cfg)
}

fn set_version(path: &Path, version: usize) {
    open_unmigrated(path)
        .unwrap()
        .execute(
            "UPDATE meta SET value = ?1 WHERE key = 'schema_version'",
            [version.to_string()],
        )
        .unwrap();
}

fn version_of(path: &Path) -> usize {
    schema_version(&open_unmigrated(path).unwrap()).unwrap()
}

/// A database with a project, as a daemon on schema `version` left it. Every
/// migration is safe to run again, so replaying the rest is the upgrade.
fn older_db(cfg: &Config, version: usize) -> Db {
    let db = Db::open(&cfg.db_path()).unwrap();
    db.create_project("acme", "acme").unwrap();
    set_version(&cfg.db_path(), version);
    db
}

fn project_names(db: &Db) -> Vec<String> {
    db.list_projects()
        .unwrap()
        .into_iter()
        .map(|p| p.name)
        .collect()
}

fn backups(cfg: &Config) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(backups_dir(cfg)) else {
        return Vec::new();
    };
    let mut out: Vec<PathBuf> = entries.map(|e| e.unwrap().path()).collect();
    out.sort();
    out
}

fn manifest(folder: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(folder.join("manifest.json")).unwrap()).unwrap()
}

#[test]
fn an_older_schema_is_backed_up_once_before_it_migrates_and_the_backup_restores() {
    let (_dir, cfg) = home();
    let (from, to) = (MIGRATIONS.len() - 2, MIGRATIONS.len());
    // Still open, as a crashed daemon leaves it: the project is in the WAL,
    // which a copy of the database file would miss.
    let live = older_db(&cfg, from);
    let wal = cfg.home.join("bus.sqlite-wal");
    assert!(std::fs::metadata(&wal).is_ok_and(|m| m.len() > 0));
    let bot = cfg.projects_dir().join("acme/bots/alice");
    std::fs::create_dir_all(&bot).unwrap();
    std::fs::write(bot.join("bot.json"), "{}").unwrap();

    let out = before_migrations(&cfg).unwrap().expect("a backup");
    assert_eq!(backups(&cfg), vec![out.clone()]);
    let name = out.file_name().unwrap().to_string_lossy().to_string();
    assert!(name.starts_with("pre-migration-"), "{name}");
    assert!(name.ends_with(&format!("-schema-{from}-to-{to}")), "{name}");
    let m = manifest(&out);
    assert_eq!(
        (&m["kind"], &m["schema_version"], &m["migrating_to"]),
        (&json!("pre-migration"), &json!(from), &json!(to))
    );
    assert_eq!(m["includes_secrets"], json!(false));
    assert!(out.join("projects/acme/bots/alice/bot.json").exists());
    // Taking it migrated nothing, the copy included.
    assert_eq!(version_of(&out.join("bus.sqlite")), from);
    assert_eq!(version_of(&cfg.db_path()), from);

    // The daemon then migrates, and its next start has nothing to back up.
    drop(live);
    drop(Db::open(&cfg.db_path()).unwrap());
    assert_eq!(version_of(&cfg.db_path()), to);
    assert_eq!(before_migrations(&cfg).unwrap(), None);
    assert_eq!(backups(&cfg).len(), 1);

    // Restored, it is the database from before the migration, and it opens.
    let (_dir2, cfg2) = home();
    restore(&cfg2, &out, false).unwrap();
    assert_eq!(version_of(&cfg2.db_path()), from);
    assert_eq!(
        version_of(&out.join("bus.sqlite")),
        from,
        "restore changed it"
    );
    assert!(cfg2
        .projects_dir()
        .join("acme/bots/alice/bot.json")
        .exists());
    let restored = Db::open(&cfg2.db_path()).unwrap();
    assert!(restored.integrity_check().unwrap());
    assert_eq!(project_names(&restored), ["acme"]);
}

#[test]
fn no_backup_for_a_fresh_database_or_one_with_nothing_pending() {
    let (_dir, cfg) = home();
    let db_path = cfg.db_path();
    assert_eq!(before_migrations(&cfg).unwrap(), None, "no database");
    std::fs::write(&db_path, b"").unwrap();
    assert_eq!(before_migrations(&cfg).unwrap(), None, "an empty file");
    std::fs::remove_file(&db_path).unwrap();
    // A file SQLite wrote before any migration ran: no tables.
    rusqlite::Connection::open(&db_path)
        .unwrap()
        .pragma_update(None, "user_version", 1)
        .unwrap();
    assert!(std::fs::metadata(&db_path).unwrap().len() > 0);
    assert_eq!(before_migrations(&cfg).unwrap(), None, "no tables");
    std::fs::remove_file(&db_path).unwrap();

    drop(Db::open(&db_path).unwrap());
    assert_eq!(before_migrations(&cfg).unwrap(), None, "nothing pending");
    // A newer daemon's database: nothing this binary would migrate.
    set_version(&db_path, MIGRATIONS.len() + 1);
    assert_eq!(before_migrations(&cfg).unwrap(), None, "newer schema");
    assert!(!backups_dir(&cfg).exists());
}

#[test]
fn a_backup_that_cannot_start_refuses_and_leaves_the_database() {
    let (_dir, cfg) = home();
    let from = MIGRATIONS.len() - 1;
    drop(older_db(&cfg, from));
    // A file where the folder goes: nothing can be written there, on any
    // platform and as any user.
    std::fs::write(backups_dir(&cfg), "not a folder").unwrap();

    let err = before_migrations(&cfg).unwrap_err().to_string();
    for part in [
        "refusing to start",
        &format!("from schema {from} to {}", MIGRATIONS.len()),
        "Nothing was migrated",
        "Free disk space",
    ] {
        assert!(err.contains(part), "{part:?} missing from: {err}");
    }
    assert_eq!(version_of(&cfg.db_path()), from);
    assert!(backups_dir(&cfg).is_file());
    let db = Db::open(&cfg.db_path()).unwrap();
    assert_eq!(project_names(&db), ["acme"]);
}

#[test]
fn a_backup_that_fails_halfway_leaves_no_folder_behind() {
    let (_dir, cfg) = home();
    let from = MIGRATIONS.len() - 1;
    drop(older_db(&cfg, from));
    // The configuration can't be copied once the database has been: the
    // folder is half written when the backup fails.
    std::fs::create_dir_all(cfg.home.join(crate::brand::daemon_file(".toml"))).unwrap();

    let err = before_migrations(&cfg).unwrap_err().to_string();
    assert!(err.contains("Nothing was migrated"), "{err}");
    assert!(backups_dir(&cfg).is_dir());
    assert_eq!(backups(&cfg), Vec::<PathBuf>::new());
    assert_eq!(version_of(&cfg.db_path()), from);
}

fn folder(dir: &Path, name: &str, kind: Option<&str>) {
    std::fs::create_dir_all(dir.join(name)).unwrap();
    let mut manifest = json!({ "version": 1 });
    if let Some(kind) = kind {
        manifest["kind"] = json!(kind);
    }
    std::fs::write(dir.join(name).join("manifest.json"), manifest.to_string()).unwrap();
}

#[test]
fn retention_keeps_the_newest_pre_migration_backups_and_nothing_else_goes() {
    let dir = tempfile::tempdir().unwrap();
    let dir = dir.path();
    let ours: Vec<String> = (1..=5)
        .map(|day| format!("pre-migration-2026100{day}-120000-schema-60-to-68"))
        .collect();
    for name in &ours {
        folder(dir, name, Some("pre-migration"));
    }
    // `hermesd backup` before manifests had a kind, and since.
    folder(dir, "backup-20250101-000000", None);
    folder(dir, "backup-20250102-000000", Some("manual"));
    // The owner's own folder, pre-migration by name only.
    folder(dir, "pre-migration-before-k9", None);
    std::fs::create_dir_all(dir.join("app")).unwrap();

    let removed = prune(dir, 3).unwrap();
    assert_eq!(removed, vec![dir.join(&ours[0]), dir.join(&ours[1])]);
    let mut left: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .collect();
    left.sort();
    let mut expected = vec![
        "app".to_string(),
        "backup-20250101-000000".to_string(),
        "backup-20250102-000000".to_string(),
        "pre-migration-before-k9".to_string(),
    ];
    expected.extend(ours[2..].iter().cloned());
    expected.sort();
    assert_eq!(left, expected);
}

#[test]
fn restarts_before_the_migration_lands_keep_only_the_newest_backups() {
    let (_dir, cfg) = home();
    drop(older_db(&cfg, MIGRATIONS.len() - 1));
    let manual = backup(&cfg, &backups_dir(&cfg).join("backup-20250101-000000")).unwrap();
    assert_eq!(manifest(&manual)["kind"], json!("manual"));

    // A daemon whose migration keeps failing backs up on every start.
    let taken: Vec<PathBuf> = (0..KEEP_PRE_MIGRATION + 2)
        .map(|_| before_migrations(&cfg).unwrap().expect("a backup"))
        .collect();
    let mut expected = vec![manual];
    expected.extend(taken[2..].iter().cloned());
    expected.sort();
    assert_eq!(backups(&cfg), expected);
}
