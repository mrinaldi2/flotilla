//! Moving files to the trash instead of erasing them (0.19.0).
//!
//! What a deleted project left on disk goes to the OS trash where this
//! computer has one the daemon can use: the macOS Trash, the Windows
//! Recycle Bin, the freedesktop trash on a Linux desktop. Otherwise, or when
//! the OS trash refuses an item (another volume, a headless box), it goes to
//! `<home>/trash/<date>-<project-dir>/`, which the daily retention empties
//! after `trash.fallback_days`. Nothing is ever erased on the way.
//!
//! Callers decide *what* may be trashed and check it with [`confine`]
//! first; this module only moves it. Test builds never reach the real OS
//! trash: they always use the folder in their own home.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::config::Config;

pub mod confine;
mod fallback;
mod os;
mod size;

pub use fallback::{root as fallback_root, sweep};
pub use size::size_of;

/// Where a trashed item went.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Bin {
    /// The OS trash: the Trash, the Recycle Bin, the freedesktop trash.
    Os,
    /// `<home>/trash`, kept `trash.fallback_days`.
    Folder,
}

/// The folder in the fallback trash an item is filed under: the project's
/// folder name, and the kind of item.
#[derive(Debug, Clone, Copy)]
pub struct Bucket<'a> {
    pub label: &'a str,
    pub group: &'a str,
}

/// Where trashed items go first on this computer.
pub fn first_bin(cfg: &Config) -> Bin {
    if os::enabled(cfg) {
        Bin::Os
    } else {
        Bin::Folder
    }
}

/// What the owner calls `bin` on this computer.
pub fn bin_name(bin: Bin) -> &'static str {
    match bin {
        Bin::Os if cfg!(windows) => "Recycle Bin",
        Bin::Os => "Trash",
        Bin::Folder => "trash folder",
    }
}

/// Moves `path`, already confined by the caller, to the OS trash or, when
/// that is unavailable or refuses it, to the trash folder in the home. An
/// error leaves `path` where it was.
pub fn move_to_trash(cfg: &Config, path: &Path, bucket: Bucket<'_>) -> Result<Bin, String> {
    if os::usable_for(cfg, path) {
        match os::trash(path) {
            Ok(()) => return Ok(Bin::Os),
            Err(error) => {
                tracing::info!(path = %path.display(), error, "the OS trash refused; using the trash folder");
            }
        }
    }
    fallback::put(cfg, path, bucket).map(|_| Bin::Folder)
}

/// The trash folder's bucket for `bucket` today, for tests and the preview.
pub fn bucket_dir(cfg: &Config, bucket: Bucket<'_>) -> PathBuf {
    fallback::bucket_dir(cfg, bucket.label).join(bucket.group)
}

#[cfg(test)]
mod tests;
