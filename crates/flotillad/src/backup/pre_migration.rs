//! The backup a starting daemon takes before it migrates the database (0.19.0
//! plan, "Data migration"). Migrations only go forward, so when the stored
//! schema is older than this binary's, the same backup `flotillad backup` takes
//! goes into `backups/` first, named for the two versions. If it can't be
//! taken, nothing is migrated and the daemon doesn't start.
//!
//! A fresh database, or one with nothing pending, is not backed up. Only the
//! newest few pre-migration backups are kept, so a daemon that keeps
//! restarting can't fill the disk; any other folder in `backups/` (the
//! owner's own backups among them) is never touched.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context};
use bus::schema::MIGRATIONS;
use serde_json::{json, Value};

use crate::config::Config;

/// How many pre-migration backups are kept, the newest first.
pub const KEEP_PRE_MIGRATION: usize = 3;

/// Every pre-migration backup's folder name starts with this.
const PREFIX: &str = "pre-migration-";
/// The manifest's `kind` for a pre-migration backup, the only kind retention
/// removes.
const KIND: &str = "pre-migration";

/// A migration a start would run: the stored schema version and this binary's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pending {
    pub from: usize,
    pub to: usize,
}

/// What opening the database at `db_path` would migrate, read without
/// migrating it. `None` for no database, an empty one, or nothing pending.
pub fn pending(db_path: &Path) -> anyhow::Result<Option<Pending>> {
    match std::fs::metadata(db_path) {
        Ok(meta) if meta.len() > 0 => {}
        Ok(_) => return Ok(None),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e).with_context(|| format!("reading {}", db_path.display())),
    }
    let conn = super::open_unmigrated(db_path)?;
    let tables: i64 = conn.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type = 'table'",
        [],
        |r| r.get(0),
    )?;
    if tables == 0 {
        return Ok(None);
    }
    let from = crate::db::schema_version(&conn)?;
    let to = MIGRATIONS.len();
    Ok((from < to).then_some(Pending { from, to }))
}

/// Before the daemon opens (and so migrates) its database: back it up when it
/// has migrations pending. Returns the backup folder, or `None` when there was
/// nothing to back up. An error means nothing was migrated and the daemon
/// must not start.
pub fn before_migrations(cfg: &Config) -> anyhow::Result<Option<PathBuf>> {
    let db_path = cfg.db_path();
    let Some(pending) = pending(&db_path)
        .with_context(|| format!("reading the schema version of {}", db_path.display()))?
    else {
        return Ok(None);
    };
    let dir = super::backups_dir(cfg);
    let out = fresh_dir(&dir, pending, chrono::Utc::now());
    if let Err(e) = take(cfg, &out, pending) {
        // Half a backup is none, and restarts must not pile them up. The
        // folder is new, so nothing else is in it.
        if out.exists() {
            let _ = std::fs::remove_dir_all(&out);
        }
        return Err(refusal(&db_path, &dir, pending, &e));
    }
    tracing::info!(
        backup = %out.display(),
        from = pending.from,
        to = pending.to,
        "backed up the database before migrating it"
    );
    match prune(&dir, KEEP_PRE_MIGRATION) {
        Ok(removed) => {
            for old in removed {
                tracing::info!(backup = %old.display(), "removed an older pre-migration backup");
            }
        }
        Err(e) => {
            tracing::warn!(error = %format!("{e:#}"), "pre-migration backup retention failed")
        }
    }
    Ok(Some(out))
}

/// `pre-migration-<UTC time>-schema-<from>-to-<to>`, with `-2`, `-3`… when a
/// start in the same second already took that name. Names sort by time.
fn fresh_dir(dir: &Path, pending: Pending, at: chrono::DateTime<chrono::Utc>) -> PathBuf {
    let base = format!(
        "{PREFIX}{}-schema-{}-to-{}",
        at.format("%Y%m%d-%H%M%S"),
        pending.from,
        pending.to
    );
    let mut out = dir.join(&base);
    let mut n = 2;
    while out.exists() {
        out = dir.join(format!("{base}-{n}"));
        n += 1;
    }
    out
}

fn take(cfg: &Config, out: &Path, pending: Pending) -> anyhow::Result<()> {
    let src = super::open_unmigrated(&cfg.db_path())?;
    let extra = json!({
        "kind": KIND,
        "schema_version": pending.from,
        "migrating_to": pending.to,
    });
    super::write(cfg, &src, out, extra)?;
    drop(src);
    // The copy must be what the restore will need: whole, and unmigrated.
    let copy = out.join("bus.sqlite");
    if !super::intact(&copy)? {
        anyhow::bail!("the copy failed SQLite's integrity check");
    }
    let copied = crate::db::schema_version(&super::open_unmigrated(&copy)?)?;
    anyhow::ensure!(
        copied == pending.from,
        "the copy is at schema {copied}, not {}",
        pending.from
    );
    Ok(())
}

fn refusal(db: &Path, dir: &Path, pending: Pending, e: &anyhow::Error) -> anyhow::Error {
    anyhow!(
        "refusing to start: could not back up {} before migrating it from schema {} to {} \
         ({e:#}). Nothing was migrated and the database is unchanged. Free disk space and \
         make sure {} is a folder flotillad can write to, then start flotillad again. To keep \
         the database as it is, reinstall the previous version instead.",
        db.display(),
        pending.from,
        pending.to,
        dir.display()
    )
}

/// Keep the newest `keep` pre-migration backups in `dir` and remove the rest.
/// Only folders whose manifest says they are pre-migration backups count, so
/// a manual backup is never removed, whatever its name. Returns what went.
fn prune(dir: &Path, keep: usize) -> anyhow::Result<Vec<PathBuf>> {
    let mut ours: Vec<PathBuf> = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        let named = path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with(PREFIX));
        if named && path.is_dir() && is_pre_migration(&path) {
            ours.push(path);
        }
    }
    ours.sort();
    let excess = ours.len().saturating_sub(keep);
    let removed: Vec<PathBuf> = ours.into_iter().take(excess).collect();
    for old in &removed {
        std::fs::remove_dir_all(old).with_context(|| format!("removing {}", old.display()))?;
    }
    Ok(removed)
}

fn is_pre_migration(folder: &Path) -> bool {
    std::fs::read_to_string(folder.join("manifest.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .is_some_and(|manifest| manifest["kind"] == KIND)
}

#[cfg(test)]
#[path = "pre_migration_tests.rs"]
mod tests;
