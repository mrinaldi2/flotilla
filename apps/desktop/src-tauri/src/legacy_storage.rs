//! Carries the webview's storage over from the bundle identifiers used
//! before the renames (`com.manuelrinaldi.thehermes`, and before that
//! `in.mikolajczuk.gravity`) to `com.manuelrinaldi.flotilla`.
//!
//! The webview keys localStorage — UI preferences, pinned bots, the saved
//! connection — by the app's identifier, so without this the first launch
//! under the new one would start from a blank setup. The newest old
//! directory that exists is copied once, before any webview exists, and only
//! while the new one does not exist yet. The old copy is left in place for a
//! downgrade. Keys keep their `hermes.`/`gravity.` names; `storage.ts` reads
//! them as fallbacks.
//!
//! macOS permission grants (microphone, speech recognition, the global
//! shortcut's accessibility access) are also keyed by identifier and cannot
//! be carried over: they are asked for again.

use std::path::{Path, PathBuf};

/// Must match `identifier` in `tauri.conf.json`.
const IDENTIFIER: &str = "com.manuelrinaldi.flotilla";
/// The identifiers before the renames, newest first.
const LEGACY_IDENTIFIERS: [&str; 2] = ["com.manuelrinaldi.thehermes", "in.mikolajczuk.gravity"];

/// Where the platform's webview keeps an app's website data.
fn webview_data_root() -> Option<PathBuf> {
    if cfg!(target_os = "macos") {
        Some(PathBuf::from(std::env::var_os("HOME")?).join("Library/WebKit"))
    } else if cfg!(windows) {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
    } else {
        let home = PathBuf::from(std::env::var_os("HOME")?);
        Some(
            std::env::var_os("XDG_DATA_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".local/share")),
        )
    }
}

/// Copies the newest old identifier's webview data, once. Failures are
/// logged and never stop the app: the cost is only the preferences.
pub fn migrate() {
    let Some(root) = webview_data_root() else {
        return;
    };
    if let Err(error) = migrate_in(&root) {
        eprintln!("could not carry webview data over to {IDENTIFIER}: {error}");
    }
}

/// The identifier whose data was copied, if any was.
fn migrate_in(root: &Path) -> std::io::Result<Option<&'static str>> {
    let new = root.join(IDENTIFIER);
    if new.exists() {
        return Ok(None);
    }
    let Some(legacy) = LEGACY_IDENTIFIERS
        .iter()
        .find(|legacy| root.join(legacy).is_dir())
    else {
        return Ok(None);
    };
    // Staged and renamed, so an interrupted copy is retried rather than
    // mistaken for a finished one.
    let staged = root.join(format!("{IDENTIFIER}.migrating"));
    let _ = std::fs::remove_dir_all(&staged);
    copy_tree(&root.join(legacy), &staged)?;
    std::fs::rename(&staged, &new)?;
    Ok(Some(legacy))
}

/// The app Flotilla replaced, while it is still installed. Flotilla has its
/// own bundle identifier, so macOS keeps The Hermes beside it rather than
/// replacing it; the app tells the owner it can go (its data was copied
/// above, and the service install took over its daemon). Never deleted from
/// here: the owner moves it to the Trash. Windows needs none of this, since
/// the installer takes the old install over.
pub fn installed_app() -> Option<PathBuf> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    let home = std::env::var_os("HOME").map(PathBuf::from);
    installed_app_in(Path::new("/Applications"), home.as_deref())
}

fn installed_app_in(applications: &Path, user_home: Option<&Path>) -> Option<PathBuf> {
    let user_apps = user_home.map(|home| home.join("Applications"));
    ["The Hermes.app", "Gravity.app"]
        .iter()
        .flat_map(|name| {
            std::iter::once(applications.join(name))
                .chain(user_apps.as_ref().map(|dir| dir.join(name)))
        })
        .find(|app| app.join("Contents").is_dir())
}

fn copy_tree(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let target = dst.join(entry.file_name());
        if kind.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else if kind.is_file() {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root(name: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("flotilla-webkit-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        root
    }

    fn prefs(root: &Path, identifier: &str, text: &[u8]) -> PathBuf {
        let local = root
            .join(identifier)
            .join("WebsiteData/Default/LocalStorage");
        std::fs::create_dir_all(&local).expect("old storage");
        std::fs::write(local.join("localstorage.sqlite3"), text).expect("prefs");
        local.join("localstorage.sqlite3")
    }

    #[test]
    fn copies_the_old_identifiers_data_once() {
        let root = root("once");
        let old = prefs(&root, LEGACY_IDENTIFIERS[0], b"prefs");

        assert_eq!(migrate_in(&root.join("absent")).expect("nothing"), None);
        assert_eq!(
            migrate_in(&root).expect("copy"),
            Some(LEGACY_IDENTIFIERS[0])
        );
        let copied = root
            .join(IDENTIFIER)
            .join("WebsiteData/Default/LocalStorage/localstorage.sqlite3");
        assert_eq!(std::fs::read(&copied).expect("copied prefs"), b"prefs");
        assert!(old.exists(), "old data kept");

        // A second launch, after the app wrote under the new identifier,
        // must not overwrite it.
        std::fs::write(&copied, b"newer").expect("newer prefs");
        assert_eq!(migrate_in(&root).expect("second launch"), None);
        assert_eq!(std::fs::read(&copied).expect("kept"), b"newer");
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn finds_the_replaced_app_in_either_applications_folder() {
        let root = root("apps");
        let (system, home) = (root.join("Applications"), root.join("home"));
        assert_eq!(installed_app_in(&system, Some(&home)), None);
        let user_app = home.join("Applications/The Hermes.app");
        std::fs::create_dir_all(user_app.join("Contents")).expect("user app");
        assert_eq!(installed_app_in(&system, Some(&home)), Some(user_app));
        let app = system.join("The Hermes.app");
        std::fs::create_dir_all(app.join("Contents")).expect("app");
        assert_eq!(installed_app_in(&system, Some(&home)), Some(app));
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    /// The Hermes's data is newer than Gravity's, which it was copied from.
    #[test]
    fn the_newest_old_identifier_wins() {
        let root = root("newest");
        prefs(&root, LEGACY_IDENTIFIERS[1], b"gravity");
        prefs(&root, LEGACY_IDENTIFIERS[0], b"hermes");
        assert_eq!(
            migrate_in(&root).expect("copy"),
            Some(LEGACY_IDENTIFIERS[0])
        );
        let copied = root
            .join(IDENTIFIER)
            .join("WebsiteData/Default/LocalStorage/localstorage.sqlite3");
        assert_eq!(std::fs::read(copied).expect("copied"), b"hermes");
        std::fs::remove_dir_all(root).expect("cleanup");
    }
}
