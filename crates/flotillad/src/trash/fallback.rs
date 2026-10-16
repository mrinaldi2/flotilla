//! The trash folder in the home, `<home>/trash/<date>-<project-dir>/<group>/`,
//! for when the OS trash is unavailable or refuses an item. Items are only
//! renamed into it, so nothing moves between volumes: an item on another
//! volume than the home stays where it is, reported. The daily retention
//! removes a bucket once its date is `trash.fallback_days` old.

use std::path::{Path, PathBuf};

use chrono::{NaiveDate, Utc};

use super::Bucket;
use crate::config::Config;

/// The trash folder of this home.
pub fn root(cfg: &Config) -> PathBuf {
    cfg.home.join("trash")
}

/// Today's bucket for `label`: `<home>/trash/<YYYY-MM-DD>-<label>`.
pub(super) fn bucket_dir(cfg: &Config, label: &str) -> PathBuf {
    let label: String = label
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '-'
            }
        })
        .collect();
    let label = label.trim_matches('.');
    let label = if label.is_empty() { "items" } else { label };
    root(cfg).join(format!("{}-{label}", Utc::now().format("%Y-%m-%d")))
}

/// A folder made here, not reached through a link a bot planted.
fn plain_dir(path: &Path) -> Result<(), String> {
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.is_dir() && !crate::cleanup::scope::is_link(&meta) => Ok(()),
        Ok(_) => Err(format!("{} is not a plain folder", path.display())),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

/// Renames `path` into today's bucket, under a name not taken yet.
pub(super) fn put(cfg: &Config, path: &Path, bucket: Bucket<'_>) -> Result<PathBuf, String> {
    let day = bucket_dir(cfg, bucket.label);
    let dir = day.join(bucket.group);
    std::fs::create_dir_all(&dir).map_err(|e| format!("making {}: {e}", dir.display()))?;
    for each in [&root(cfg), &day, &dir] {
        plain_dir(each)?;
    }
    let name = path
        .file_name()
        .ok_or_else(|| format!("{} has no name", path.display()))?;
    let mut to = dir.join(name);
    let mut n = 2;
    while std::fs::symlink_metadata(&to).is_ok() {
        let mut numbered = name.to_os_string();
        numbered.push(format!("-{n}"));
        to = dir.join(numbered);
        n += 1;
    }
    std::fs::rename(path, &to).map_err(|e| format!("moving to {}: {e}", to.display()))?;
    tracing::info!(from = %path.display(), to = %to.display(), "moved to the trash folder");
    Ok(to)
}

/// The day a bucket was filled, from its name.
fn bucket_day(name: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(name.get(..10)?, "%Y-%m-%d").ok()
}

/// Removes the buckets older than `trash.fallback_days`. Returns how many
/// went. Only plain folders directly in the trash folder, named by a date,
/// are ever removed; anything else there is left alone.
pub fn sweep(cfg: &Config) -> usize {
    let days = cfg.trash.fallback_days.max(1);
    let cutoff = Utc::now().date_naive() - chrono::Duration::days(days);
    let Ok(entries) = std::fs::read_dir(root(cfg)) else {
        return 0;
    };
    let mut removed = 0;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(day) = bucket_day(&name) else {
            continue;
        };
        if day >= cutoff || plain_dir(&entry.path()).is_err() {
            continue;
        }
        match std::fs::remove_dir_all(entry.path()) {
            Ok(()) => {
                removed += 1;
                tracing::info!(bucket = %name, "trash folder bucket emptied");
            }
            Err(e) => tracing::warn!(bucket = %name, error = %e, "emptying a trash bucket failed"),
        }
    }
    removed
}
