//! The OS trash, through the `trash` crate.
//!
//! - macOS: `NSFileManager trashItemAtURL`, never the Finder (asking the
//!   Finder over AppleScript needs an Automation grant a daemon can't ask
//!   for). An item on a volume with no Trash fails, and goes to the folder.
//! - Windows: the shell's `IFileOperation` with undo allowed and no UI, only
//!   for items on a local fixed drive: a network or removable drive has no
//!   Recycle Bin, and the shell would delete outright.
//! - Linux and other freedesktop systems: the home trash (or a mount's
//!   `.Trash-<uid>`), only where a desktop session or an existing trash
//!   shows someone will see it: a headless server gets the folder, which is
//!   emptied on schedule, rather than a trash nobody ever empties.

use std::path::Path;

use crate::config::Config;

/// Whether the OS trash is tried at all on this computer.
pub(super) fn enabled(cfg: &Config) -> bool {
    // Test daemons and unit tests never touch the real trash of whoever
    // runs them: everything goes to the folder in their own home.
    if cfg!(any(test, feature = "test-support")) {
        return false;
    }
    cfg.trash.os_trash && desktop_trash(&cfg.user_home)
}

/// Whether the OS trash is tried for `path`.
pub(super) fn usable_for(cfg: &Config, path: &Path) -> bool {
    enabled(cfg) && recyclable_drive(path)
}

/// Moves `path` to the OS trash.
pub(super) fn trash(path: &Path) -> Result<(), String> {
    // The crate unwraps a COM call on Windows: a panic there must leave the
    // item where it was, not take the daemon's thread down.
    match std::panic::catch_unwind(|| context().delete(path)) {
        Ok(Ok(())) => Ok(()),
        Ok(Err(error)) => Err(error.to_string()),
        Err(_) => Err("the OS trash failed unexpectedly".to_string()),
    }
}

#[cfg(target_os = "macos")]
fn context() -> trash::TrashContext {
    use trash::macos::{DeleteMethod, TrashContextExtMacos};
    let mut context = trash::TrashContext::default();
    context.set_delete_method(DeleteMethod::NsFileManager);
    context
}

#[cfg(not(target_os = "macos"))]
fn context() -> trash::TrashContext {
    trash::TrashContext::default()
}

/// A freedesktop trash someone will see: a desktop session in the
/// daemon's environment, or a trash that a file manager already made.
#[cfg(all(unix, not(target_os = "macos")))]
fn desktop_trash(user_home: &Path) -> bool {
    let set = |name: &str| std::env::var_os(name).is_some_and(|v| !v.is_empty());
    if set("XDG_CURRENT_DESKTOP") || set("WAYLAND_DISPLAY") || set("DISPLAY") {
        return true;
    }
    let data = std::env::var_os("XDG_DATA_HOME")
        .filter(|v| !v.is_empty())
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| user_home.join(".local").join("share"));
    data.join("Trash").is_dir()
}

/// macOS and Windows always have one for the signed-in user.
#[cfg(not(all(unix, not(target_os = "macos"))))]
fn desktop_trash(_user_home: &Path) -> bool {
    true
}

/// Windows recycles only on a local fixed drive.
#[cfg(windows)]
fn recyclable_drive(path: &Path) -> bool {
    use std::os::windows::ffi::OsStrExt;
    use std::path::{Component, Prefix};

    #[link(name = "kernel32")]
    extern "system" {
        fn GetDriveTypeW(root: *const u16) -> u32;
    }
    const DRIVE_FIXED: u32 = 3;

    let Ok(real) = std::fs::canonicalize(path) else {
        return false;
    };
    let letter = match real.components().next() {
        Some(Component::Prefix(prefix)) => match prefix.kind() {
            Prefix::VerbatimDisk(letter) | Prefix::Disk(letter) => letter,
            _ => return false,
        },
        _ => return false,
    };
    let root: Vec<u16> = std::ffi::OsStr::new(&format!("{}:\\", char::from(letter)))
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    // SAFETY: `root` is a NUL-terminated UTF-16 string that outlives the
    // call, and GetDriveTypeW only reads it.
    unsafe { GetDriveTypeW(root.as_ptr()) == DRIVE_FIXED }
}

/// Elsewhere the trash itself says when a volume has none.
#[cfg(not(windows))]
fn recyclable_drive(_path: &Path) -> bool {
    true
}
