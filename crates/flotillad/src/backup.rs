//! Online backup and restore: SQLite backup API plus configuration
//! manifests. Secrets are never included.
//!
//! A backup reads the database as it is on disk: it never migrates it, so a
//! newer `flotillad` backs up what an older one wrote, and restoring never
//! changes the backup it restores from.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context};
use rusqlite::{Connection, OpenFlags};
use serde_json::{json, Value};

use crate::config::Config;

mod pre_migration;

pub use pre_migration::{before_migrations, pending, Pending, KEEP_PRE_MIGRATION};

/// Where backups go unless `--out` names another folder.
pub fn backups_dir(cfg: &Config) -> PathBuf {
    cfg.home.join("backups")
}

/// Create a backup directory containing `bus.sqlite`, `flotillad.toml` (if any),
/// per-project/bot config files, and a manifest. Secrets and workspaces are
/// excluded.
pub fn backup(cfg: &Config, out: &Path) -> anyhow::Result<PathBuf> {
    let db_path = cfg.db_path();
    if !db_path.exists() {
        bail!("no database at {}; nothing to back up", db_path.display());
    }
    let src = open_unmigrated(&db_path)?;
    let version = crate::db::schema_version(&src)?;
    write(
        cfg,
        &src,
        out,
        json!({ "kind": "manual", "schema_version": version }),
    )
}

/// The database as it is, without the migrations `Db::open` runs, and
/// without creating it.
pub(crate) fn open_unmigrated(path: &Path) -> anyhow::Result<Connection> {
    let flags = OpenFlags::SQLITE_OPEN_READ_WRITE
        | OpenFlags::SQLITE_OPEN_URI
        | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let conn = Connection::open_with_flags(path, flags)
        .with_context(|| format!("open {}", path.display()))?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    Ok(conn)
}

/// Write the backup folder from `src`; `extra` joins the manifest.
fn write(cfg: &Config, src: &Connection, out: &Path, extra: Value) -> anyhow::Result<PathBuf> {
    if out.exists() && out.read_dir()?.next().is_some() {
        bail!("backup destination is not empty: {}", out.display());
    }
    std::fs::create_dir_all(out)?;

    copy_db(src, &out.join("bus.sqlite")).context("SQLite online backup")?;

    let cfg_name = crate::brand::daemon_file(".toml");
    let cfg_path = cfg.home.join(&cfg_name);
    if cfg_path.exists() {
        std::fs::copy(&cfg_path, out.join(&cfg_name))?;
    }

    // Bot configuration files, not workspaces and never `secrets/`.
    let mut copied = 0usize;
    let projects_src = cfg.projects_dir();
    if projects_src.exists() {
        copied = copy_config_tree(&projects_src, &out.join("projects"))?;
    }

    let mut manifest = json!({
        "version": 1,
        "created_at": chrono::Utc::now().to_rfc3339(),
        "daemon_version": crate::app::DAEMON_VERSION,
        "config_files": copied,
        "includes_secrets": false
    });
    if let (Some(fields), Value::Object(extra)) = (manifest.as_object_mut(), extra) {
        fields.extend(extra);
    }
    std::fs::write(
        out.join("manifest.json"),
        serde_json::to_string_pretty(&manifest)?,
    )?;
    Ok(out.to_path_buf())
}

/// SQLite's online backup: a consistent copy even of a live WAL database,
/// which a file copy is not. The copy is closed when this returns.
fn copy_db(src: &Connection, dest: &Path) -> anyhow::Result<()> {
    let mut dst = Connection::open(dest)?;
    let backup = rusqlite::backup::Backup::new(src, &mut dst)?;
    backup.run_to_completion(64, std::time::Duration::from_millis(50), None)?;
    Ok(())
}

/// `PRAGMA integrity_check` on a database file, without migrating it.
fn intact(path: &Path) -> anyhow::Result<bool> {
    let conn = open_unmigrated(path)?;
    let res: String = conn.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
    Ok(res == "ok")
}

const CONFIG_FILES: &[&str] = &["project.json", "bot.json", "system.md", "mcp.json"];

fn copy_config_tree(src: &Path, dst: &Path) -> anyhow::Result<usize> {
    let mut copied = 0;
    for entry in walk(src)? {
        let name = entry
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        if !CONFIG_FILES.contains(&name) {
            continue;
        }
        let rel = entry.strip_prefix(src)?;
        let target = dst.join(rel);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(&entry, &target)?;
        copied += 1;
    }
    Ok(copied)
}

fn walk(dir: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d)? {
            let path = entry?.path();
            if path.is_dir() {
                // Workspaces hold user data, not configuration; skip them.
                if path.file_name().and_then(|n| n.to_str()) == Some("workspace") {
                    continue;
                }
                stack.push(path);
            } else {
                out.push(path);
            }
        }
    }
    Ok(out)
}

/// Restore a backup into the daemon home. Refuses to run while a live
/// database exists unless `overwrite` is set; the existing database is moved
/// aside, never deleted.
pub fn restore(cfg: &Config, from: &Path, overwrite: bool) -> anyhow::Result<()> {
    let src_db = from.join("bus.sqlite");
    if !src_db.exists() {
        bail!("{} does not contain bus.sqlite", from.display());
    }
    // Validate the backup before touching anything, and leave it as it is:
    // the daemon migrates the restored copy when it starts.
    if !intact(&src_db).context("opening backup database")? {
        bail!("backup database failed integrity check");
    }

    let dest_db = cfg.db_path();
    if dest_db.exists() {
        if !overwrite {
            bail!(
                "{} already exists; pass --overwrite to move it aside and restore",
                dest_db.display()
            );
        }
        let aside = cfg.home.join(format!(
            "bus.sqlite.pre-restore-{}",
            chrono::Utc::now().timestamp()
        ));
        std::fs::rename(&dest_db, &aside)
            .with_context(|| format!("moving current db aside to {}", aside.display()))?;
        // WAL/SHM sidecars belong to the old database.
        for ext in ["-wal", "-shm"] {
            let side = cfg.home.join(format!("bus.sqlite{ext}"));
            if side.exists() {
                std::fs::rename(&side, cfg.home.join(format!("bus.sqlite{ext}.pre-restore")))?;
            }
        }
    }
    std::fs::create_dir_all(&cfg.home)?;
    std::fs::copy(&src_db, &dest_db)?;

    let projects_src = from.join("projects");
    if projects_src.exists() {
        for entry in walk(&projects_src)? {
            let rel = entry.strip_prefix(&projects_src)?;
            let target = cfg.projects_dir().join(rel);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(&entry, &target)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::db::Db;

    #[test]
    fn backup_and_restore_roundtrip() {
        let home = tempfile::tempdir().unwrap();
        let cfg = Config {
            home: home.path().to_path_buf(),
            ..Config::default()
        };
        let db = Db::open(&cfg.db_path()).unwrap();
        let project = db.create_project("acme", "acme").unwrap();
        db.create_bot(&project.id, "alice", "", "", "", "/tmp/a", "alice", None)
            .unwrap();

        // Fake provisioned config + a secret that must not travel.
        let bot_dir = cfg.projects_dir().join("acme/bots/alice");
        std::fs::create_dir_all(bot_dir.join("workspace")).unwrap();
        std::fs::write(bot_dir.join("bot.json"), "{}").unwrap();
        std::fs::write(bot_dir.join("workspace/CLAUDE.md"), "private").unwrap();
        std::fs::create_dir_all(cfg.secrets_dir()).unwrap();
        std::fs::write(cfg.secrets_dir().join("client.token"), "s3cret").unwrap();

        let out = home.path().join("backups/b1");
        backup(&cfg, &out).unwrap();
        assert!(out.join("bus.sqlite").exists());
        assert!(out.join("manifest.json").exists());
        assert!(out.join("projects/acme/bots/alice/bot.json").exists());
        assert!(!out.join("projects/acme/bots/alice/workspace").exists());
        // No secrets anywhere in the backup.
        for f in walk(&out).unwrap() {
            let content = std::fs::read_to_string(&f).unwrap_or_default();
            assert!(!content.contains("s3cret"), "secret leaked into {f:?}");
        }

        // Restore into a fresh home.
        let home2 = tempfile::tempdir().unwrap();
        let cfg2 = Config {
            home: home2.path().to_path_buf(),
            ..Config::default()
        };
        restore(&cfg2, &out, false).unwrap();
        let db2 = Db::open(&cfg2.db_path()).unwrap();
        let projects = db2.list_projects().unwrap();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name, "acme");
        assert_eq!(db2.list_bots(None).unwrap().len(), 1);

        // Existing database blocks restore without --overwrite.
        assert!(restore(&cfg2, &out, false).is_err());
        // Windows refuses to rename a database while SQLite holds it open.
        drop(db2);
        restore(&cfg2, &out, true).unwrap();
        assert!(std::fs::read_dir(home2.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .any(|e| e
                .file_name()
                .to_string_lossy()
                .starts_with("bus.sqlite.pre-restore")));
    }
}
