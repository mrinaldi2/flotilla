//! Where the local daemon lives, mirroring `crates/flotillad/src/brand.rs`,
//! `service.rs` and `migrate_home/hop.rs`, before and after the home
//! migrations: `~/.gravity` → `~/.thehermes` → `~/.flotilla`, `gravityd.*` →
//! `hermesd.*` → `flotillad.*`, launchd label `in.mikolajczuk.gravityd` →
//! `com.manuelrinaldi.thehermesd` → `com.manuelrinaldi.flotillad`.
//!
//! The app reads every layout: on the first launch after an update the old
//! one is still on disk, and the bundled `flotillad service install` is what
//! migrates it.

use std::path::{Path, PathBuf};

/// Must match `brand::LAUNCHD_LABEL` in `crates/flotillad`.
const LAUNCHD_LABEL: &str = "com.manuelrinaldi.flotillad";
/// Must match `brand::LEGACY_LAUNCHD_LABELS` in `crates/flotillad`.
const LEGACY_LAUNCHD_LABELS: [&str; 2] =
    ["com.manuelrinaldi.thehermesd", "in.mikolajczuk.gravityd"];

/// Must match `brand::HOME_DIR_NAME` and `LEGACY_HOME_DIR_NAMES`.
const HOME_DIR_NAME: &str = ".flotilla";
const LEGACY_HOME_DIR_NAMES: [&str; 2] = [".thehermes", ".gravity"];
/// Must match `brand::DAEMON_FILE_STEM` and `LEGACY_DAEMON_FILE_STEMS`.
const FILE_STEM: &str = "flotillad";
const LEGACY_FILE_STEMS: [&str; 2] = ["hermesd", "gravityd"];
/// The home variables, newest first (`brand::all_env_names("HOME")`).
const HOME_VARS: [&str; 3] = ["FLOTILLA_HOME", "THEHERMES_HOME", "GRAVITY_HOME"];

fn file_stems() -> impl Iterator<Item = &'static str> {
    std::iter::once(FILE_STEM).chain(LEGACY_FILE_STEMS)
}

pub(crate) fn user_home() -> Result<PathBuf, String> {
    let variable = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(variable)
        .map(PathBuf::from)
        .ok_or_else(|| format!("{variable} is not set"))
}

/// Root of the daemon's state: `FLOTILLA_HOME`, then `THEHERMES_HOME`, then
/// `GRAVITY_HOME`, then the default home, or a pre-rename one while it has
/// not been migrated.
pub(crate) fn daemon_home() -> Result<PathBuf, String> {
    if let Some(home) = HOME_VARS.iter().find_map(std::env::var_os) {
        return Ok(PathBuf::from(home));
    }
    Ok(default_home(&user_home()?))
}

fn default_home(user_home: &Path) -> PathBuf {
    let home = user_home.join(HOME_DIR_NAME);
    if home.exists() {
        return home;
    }
    LEGACY_HOME_DIR_NAMES
        .iter()
        .map(|name| user_home.join(name))
        .find(|legacy| legacy.is_dir())
        .unwrap_or(home)
}

/// Mirrors `flotillad`'s `migrate_home::pending` for the default home: a run
/// of its own that stopped partway, or else a real pre-rename home holding a
/// database. Installing the service then moves the home and restarts every
/// bot, so the app asks first. A home set by environment is migrated by
/// hand, never by install.
pub(crate) fn migration_pending(user_home: &Path) -> bool {
    if HOME_VARS
        .iter()
        .any(|name| std::env::var_os(name).is_some())
    {
        return false;
    }
    migration_pending_in(user_home)
}

fn migration_pending_in(user_home: &Path) -> bool {
    let home = user_home.join(HOME_DIR_NAME);
    let legacies: Vec<PathBuf> = LEGACY_HOME_DIR_NAMES
        .iter()
        .map(|name| user_home.join(name))
        .collect();
    // The log of a move to this home; an earlier rename's log (left in the
    // home this move starts from) is not one.
    for legacy in &legacies {
        let state = [&home, legacy]
            .iter()
            .map(|dir| dir.join("migrate-home.json"))
            .find(|path| state_is_for(path, legacy, &home));
        if let Some(state) = state {
            return !migration_completed(&state);
        }
    }
    legacies.iter().any(|legacy| {
        legacy
            .symlink_metadata()
            .is_ok_and(|meta| meta.is_dir() && !meta.file_type().is_symlink())
            && legacy.join("bus.sqlite").is_file()
    })
}

fn read_state(state: &Path) -> Option<serde_json::Value> {
    std::fs::read_to_string(state)
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
}

/// Whether the `migrate-home.json` at `state` logs the move `from` → `to`.
fn state_is_for(state: &Path, from: &Path, to: &Path) -> bool {
    read_state(state).is_some_and(|value| {
        let named = |key: &str, path: &Path| {
            value[key]
                .as_str()
                .is_some_and(|text| same_home(Path::new(text), path))
        };
        named("from", from) && named("to", to)
    })
}

/// The same home, as given or with its parent resolved through links,
/// ignoring case where the file system does (`migrate_home::hop::same_home`).
fn same_home(a: &Path, b: &Path) -> bool {
    let key = |path: &Path| -> String {
        let resolved = path
            .parent()
            .and_then(|parent| parent.canonicalize().ok())
            .zip(path.file_name())
            .map_or_else(|| path.to_path_buf(), |(parent, name)| parent.join(name));
        let text = resolved.to_string_lossy().replace('\\', "/");
        if cfg!(any(windows, target_os = "macos")) {
            text.to_lowercase()
        } else {
            text
        }
    };
    a == b || key(a) == key(b)
}

/// Whether the `migrate-home.json` at `state` records a finished run.
fn migration_completed(state: &Path) -> bool {
    read_state(state).is_some_and(|value| !value["completed_at"].is_null())
}

/// An install that died between the home migration and its commit: the
/// home has moved (its `migrate-home.json` complete), a pre-rename service
/// is still registered, and the current service or its binary is missing.
/// `service install --no-migrate` finishes the switch-over; nothing moves.
pub(crate) fn switch_over_interrupted(home: &Path, user_home: &Path) -> bool {
    let markers = managed_markers(home, user_home);
    let binary = home.join(format!("bin/{FILE_STEM}{}", std::env::consts::EXE_SUFFIX));
    migration_completed(&home.join("migrate-home.json"))
        && markers[1..].iter().any(|legacy| legacy.is_file())
        && !(markers[0].is_file() && binary.is_file())
}

/// `flotillad<suffix>` in `home`, or the newest pre-rename file
/// (`hermesd<suffix>`, `gravityd<suffix>`) when only that one is there.
pub(crate) fn daemon_file(home: &Path, suffix: &str) -> PathBuf {
    let current = home.join(format!("{FILE_STEM}{suffix}"));
    if current.exists() {
        return current;
    }
    LEGACY_FILE_STEMS
        .iter()
        .map(|stem| home.join(format!("{stem}{suffix}")))
        .find(|legacy| legacy.exists())
        .unwrap_or(current)
}

fn any_managed_binary(home: &Path) -> bool {
    file_stems().any(|name| {
        home.join(format!("bin/{name}{}", std::env::consts::EXE_SUFFIX))
            .is_file()
    })
}

/// Mirrors `ServicePaths::bin_path`, `legacy_bin_paths`, `plist_path` and
/// `legacy_plist_paths`. An install from before a rename still counts, so
/// the app's update reaches it and `service install` migrates it.
pub(crate) fn managed_daemon_is_installed(home: &Path, user_home: &Path) -> bool {
    any_managed_binary(home) && managed_marker_exists(home, user_home)
}

/// The service definition survived but its binary did not (an interrupted
/// copy, a quarantine): the task or agent has nothing to start.
pub(crate) fn managed_daemon_needs_repair(home: &Path, user_home: &Path) -> bool {
    managed_marker_exists(home, user_home) && !any_managed_binary(home)
}

pub(crate) fn managed_marker_exists(home: &Path, user_home: &Path) -> bool {
    managed_markers(home, user_home)
        .iter()
        .any(|marker| marker.is_file())
}

/// The service definitions, the current one first, then the pre-rename ones
/// newest first.
pub(crate) fn managed_markers(home: &Path, user_home: &Path) -> Vec<PathBuf> {
    if cfg!(windows) {
        file_stems()
            .map(|stem| home.join(format!("{stem}-task.xml")))
            .collect()
    } else {
        let agents = user_home.join("Library/LaunchAgents");
        std::iter::once(LAUNCHD_LABEL)
            .chain(LEGACY_LAUNCHD_LABELS)
            .map(|label| agents.join(format!("{label}.plist")))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "flotilla-desktop-home-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("temporary root");
        root
    }

    #[test]
    fn an_unmigrated_home_is_used_until_the_new_one_exists() {
        let root = temp("default");
        assert_eq!(default_home(&root), root.join(".flotilla"));
        std::fs::create_dir_all(root.join(".gravity")).expect("oldest home");
        assert_eq!(default_home(&root), root.join(".gravity"));
        std::fs::create_dir_all(root.join(".thehermes")).expect("old home");
        assert_eq!(default_home(&root), root.join(".thehermes"));
        std::fs::create_dir_all(root.join(".flotilla")).expect("new home");
        assert_eq!(default_home(&root), root.join(".flotilla"));
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn daemon_files_fall_back_to_their_old_names() {
        let root = temp("files");
        assert_eq!(daemon_file(&root, ".port"), root.join("flotillad.port"));
        std::fs::write(root.join("gravityd.port"), "0\n").expect("oldest port");
        assert_eq!(daemon_file(&root, ".port"), root.join("gravityd.port"));
        std::fs::write(root.join("hermesd.port"), "1\n").expect("old port");
        assert_eq!(daemon_file(&root, ".port"), root.join("hermesd.port"));
        std::fs::write(root.join("flotillad.port"), "2\n").expect("new port");
        assert_eq!(daemon_file(&root, ".port"), root.join("flotillad.port"));
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    fn state(dir: &Path, from: &Path, to: &Path, completed: bool) {
        let text = serde_json::json!({
            "from": from, "to": to,
            "completed_at": completed.then_some("now"),
        });
        std::fs::write(dir.join("migrate-home.json"), text.to_string()).expect("state");
    }

    #[test]
    fn a_real_old_home_or_an_unfinished_run_is_a_pending_migration() {
        let root = temp("pending");
        assert!(!migration_pending_in(&root));
        let legacy = root.join(".thehermes");
        std::fs::create_dir_all(&legacy).expect("old home");
        assert!(!migration_pending_in(&root), "no database, nothing to move");
        std::fs::write(legacy.join("bus.sqlite"), b"db").expect("db");
        assert!(migration_pending_in(&root));

        let new = root.join(".flotilla");
        std::fs::rename(&legacy, &new).expect("moved");
        state(&new, &legacy, &new, false);
        assert!(migration_pending_in(&root));
        state(&new, &legacy, &new, true);
        assert!(!migration_pending_in(&root));
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    /// The Gravity → The Hermes move's finished log, still in `~/.thehermes`,
    /// says nothing about the move to `~/.flotilla`.
    #[test]
    fn an_earlier_renames_log_is_not_this_moves() {
        let root = temp("hop");
        let (gravity, hermes) = (root.join(".gravity"), root.join(".thehermes"));
        std::fs::create_dir_all(&hermes).expect("home");
        std::fs::write(hermes.join("bus.sqlite"), b"db").expect("db");
        state(&hermes, &gravity, &hermes, true);
        assert!(migration_pending_in(&root));
        assert!(!switch_over_interrupted(&hermes, &root));
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn either_label_marks_a_managed_install() {
        let root = temp("markers");
        let (home, user_home) = (root.join("h"), root.join("u"));
        let bin = home.join(format!("bin/flotillad{}", std::env::consts::EXE_SUFFIX));
        std::fs::create_dir_all(bin.parent().expect("bin dir")).expect("bin dir");
        std::fs::write(&bin, b"daemon").expect("binary");
        assert!(!managed_daemon_is_installed(&home, &user_home));
        for marker in managed_markers(&home, &user_home) {
            std::fs::create_dir_all(marker.parent().expect("parent")).expect("marker dir");
            std::fs::write(&marker, b"marker").expect("marker");
            assert!(managed_daemon_is_installed(&home, &user_home));
            std::fs::remove_file(&marker).expect("remove marker");
        }
        std::fs::remove_dir_all(root).expect("cleanup");
    }
}
