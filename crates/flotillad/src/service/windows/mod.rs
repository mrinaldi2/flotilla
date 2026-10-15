//! Per-user Task Scheduler installation; no administrator privileges required.
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Context;

mod host;
mod process_tree;
mod reap;
#[path = "../sequence.rs"]
mod sequence;
#[path = "../stage.rs"]
mod stage;
mod task;

use host::{Schtasks, System, TaskScheduler};
use sequence::{HomeMigration, Host, Layout, Migration};
use stage::{default_legacy_homes, remove_if_present, same_path, with_suffix};

pub const SERVICE_LABEL: &str = crate::brand::WINDOWS_TASK;
const DEFAULT_CONFIG: &str = include_str!("../../../../../ops/flotillad.example.toml");

#[derive(Clone)]
pub struct ServicePaths {
    home: PathBuf,
    user_home: PathBuf,
    /// Whether `home` was set with `FLOTILLA_HOME`.
    home_overridden: bool,
    /// The daemon the installer put in Program Files, when that is what
    /// installs: the task then runs it rather than the home's copy.
    installed: Option<PathBuf>,
    /// The Program Files folders of releases before the rename.
    legacy_app_dirs: Vec<PathBuf>,
}

impl ServicePaths {
    pub fn new(home: PathBuf, user_home: PathBuf) -> Self {
        Self {
            home,
            user_home,
            home_overridden: crate::config::home_is_overridden(),
            installed: None,
            legacy_app_dirs: crate::bus_auth::windows_legacy_app_dirs(),
        }
    }
    /// These paths, with the task running `source` when it is the daemon in
    /// `Program Files\Flotilla`, which only an administrator can replace,
    /// instead of the home's copy, which every bot can write (ARCH-R38). The
    /// home copy is still staged: it keeps the upgrade sequence, `status`
    /// and a downgrade to a per-user release working.
    fn running(&self, source: &Path) -> Self {
        let installed = crate::bus_auth::windows_app_dir()
            .filter(|dir| crate::bus_auth::app_identity::is_installed_daemon(source, dir))
            .map(|_| source.to_path_buf());
        Self {
            installed,
            ..self.clone()
        }
    }
    #[cfg(test)]
    fn with_installed(self, installed: PathBuf) -> Self {
        Self {
            installed: Some(installed),
            ..self
        }
    }
    /// The binary the task launches.
    fn run_bin(&self) -> PathBuf {
        self.installed.clone().unwrap_or_else(|| self.bin_path())
    }
    /// What a daemon of the task in `home` may run from: the home's copies,
    /// the installed daemon, and the copy the installer renamed aside
    /// because it was running (`installer-hooks.nsh`).
    ///
    /// A task from before the rename may run the daemon The Hermes installed
    /// in `Program Files\The Hermes` (or its copy renamed aside), so those
    /// count too: the stop must be able to end it.
    fn daemon_executables(&self, home: &Path) -> Vec<PathBuf> {
        let bin = home.join("bin");
        let mut executables: Vec<PathBuf> = crate::brand::all_daemon_files(".exe")
            .into_iter()
            .map(|name| bin.join(name))
            .collect();
        if let Some(installed) = &self.installed {
            executables.push(installed.clone());
            executables.push(with_suffix(installed, ".old"));
        }
        for dir in &self.legacy_app_dirs {
            for name in crate::brand::legacy_daemon_files(".exe") {
                let daemon = dir.join(name);
                executables.push(with_suffix(&daemon, ".old"));
                executables.push(daemon);
            }
        }
        executables
    }
    #[cfg(test)]
    fn with_home_overridden(self, home_overridden: bool) -> Self {
        Self {
            home_overridden,
            ..self
        }
    }
    pub fn bin_path(&self) -> PathBuf {
        self.home.join("bin/flotillad.exe")
    }
    /// Where releases before the renames installed the binary
    /// (`hermesd.exe`, `gravityd.exe`); a task created by one of them is
    /// still running it during an upgrade.
    pub fn legacy_bin_paths(&self) -> Vec<PathBuf> {
        crate::brand::legacy_daemon_files(".exe")
            .into_iter()
            .map(|name| self.home.join("bin").join(name))
            .collect()
    }
    // Kept as the common installation-marker API for the existing CLI.
    pub fn plist_path(&self) -> PathBuf {
        self.home.join(crate::brand::daemon_file("-task.xml"))
    }
    pub fn config_path(&self) -> PathBuf {
        self.home.join(crate::brand::daemon_file(".toml"))
    }
    pub fn log_dir(&self) -> PathBuf {
        self.home.join("logs")
    }
    fn launcher_path(&self) -> PathBuf {
        self.home.join(crate::brand::daemon_file("-task.ps1"))
    }
    fn pid_path(&self) -> PathBuf {
        self.home.join(crate::brand::daemon_file("-task.pid"))
    }
    /// Homes a task from before the renames may run from: this one, when
    /// it was set explicitly, and the default pre-rename homes unless this
    /// one is overridden (see [`default_legacy_homes`]).
    fn legacy_homes(&self) -> Vec<PathBuf> {
        let mut homes = vec![self.home.clone()];
        homes.extend(
            default_legacy_homes(&self.user_home, self.home_overridden)
                .into_iter()
                .filter(|default| *default != self.home),
        );
        homes
    }
}

/// Where an install puts things, with the daemon running from `old_home` now.
fn layout(paths: &ServicePaths, old_home: &Path) -> Layout {
    Layout {
        from_bin: old_home.join("bin/flotillad.exe"),
        bin: paths.bin_path(),
        // The pre-rename binary a task from before the rename ran; the new
        // launcher starts `flotillad.exe`.
        leftovers: paths.legacy_bin_paths(),
        port_file: crate::home::runtime_port_path(&paths.home),
        dirs: vec![paths.home.join("bin"), paths.log_dir()],
        config: paths.config_path(),
        default_config: DEFAULT_CONFIG,
    }
}

/// `service install`: installs `source` and starts it, moving the home first
/// when `migration` is given; see [`sequence`] for the order and the
/// rollback. Whatever ran before keeps running if anything fails.
pub fn install_and_start(
    source: &Path,
    paths: &ServicePaths,
    configured_port: u16,
    migration: Option<&crate::migrate_home::Plan>,
) -> anyhow::Result<()> {
    let paths = &paths.running(source);
    let old_home = migration.map_or_else(|| paths.home.clone(), |plan| plan.from.clone());
    let host = TaskScheduler::new(paths, &old_home, configured_port, System)?;
    install_with(source, paths, &old_home, migration, &host)
}

fn install_with<S: Schtasks>(
    source: &Path,
    paths: &ServicePaths,
    old_home: &Path,
    migration: Option<&crate::migrate_home::Plan>,
    host: &TaskScheduler<'_, S>,
) -> anyhow::Result<()> {
    let migration = migration.map(HomeMigration);
    sequence::upgrade(
        source,
        &layout(paths, old_home),
        host,
        migration.as_ref().map(|m| m as &dyn Migration),
    )
}

/// `service restart`. Run by the app's bundled sidecar, so a missing managed
/// binary is reinstalled from the copy that ships with the app. Never
/// migrates the home.
pub fn restart(paths: &ServicePaths, configured_port: u16) -> anyhow::Result<()> {
    let bundled = std::env::current_exe().context("locating the bundled daemon")?;
    let paths = &paths.running(&bundled);
    let host = TaskScheduler::new(paths, &paths.home, configured_port, System)?;
    sequence::restart(&bundled, &layout(paths, &paths.home), &host)
}

/// Removes the tasks (current and pre-rename) and the managed binaries.
/// Daemon state is deliberately left in place.
pub fn uninstall(paths: &ServicePaths) -> anyhow::Result<()> {
    // The app may be removed after the user already uninstalled its daemon.
    if !paths.home.is_dir() {
        return Ok(());
    }
    // The installer's uninstall runs the installed daemon itself.
    let paths = &paths.running(&std::env::current_exe().unwrap_or_default());
    let host = TaskScheduler::new(paths, &paths.home, 0, System)?;
    remove_all(paths, &host).map_err(|error| host.explain(error))
}

fn remove_all<S: Schtasks>(
    paths: &ServicePaths,
    host: &TaskScheduler<'_, S>,
) -> anyhow::Result<()> {
    for id in host.installed() {
        host.disable(id)?;
        host.stop(id)?;
        host.remove(id)?;
    }
    for path in [
        paths.plist_path(),
        paths.launcher_path(),
        paths.bin_path(),
        with_suffix(&paths.bin_path(), ".new"),
        with_suffix(&paths.bin_path(), ".old"),
        with_suffix(&paths.launcher_path(), ".old"),
        with_suffix(&paths.plist_path(), ".old"),
        paths.pid_path(),
        crate::home::runtime_port_path(&paths.home),
    ]
    .into_iter()
    .chain(paths.legacy_bin_paths())
    {
        remove_if_present(&path)?;
    }
    Ok(())
}

/// `service status --json`. Task Scheduler is not asked whether the task
/// runs, so `/health` alone says whether a daemon answers.
pub fn report(
    paths: &ServicePaths,
    configured_port: u16,
    home: crate::service_report::HomeState,
) -> crate::service_report::Report {
    let port = crate::home::runtime_port(&paths.home).unwrap_or(configured_port);
    let legacy_markers = crate::brand::legacy_daemon_files("-task.xml");
    crate::service_report::Report {
        binary: paths.bin_path().is_file() || paths.legacy_bin_paths().iter().any(|b| b.is_file()),
        service: paths.plist_path().is_file(),
        service_running: None,
        legacy_service: paths.legacy_homes().iter().any(|home| {
            legacy_markers
                .iter()
                .any(|marker| home.join(marker).is_file())
        }),
        legacy_running: None,
        migration_pending: home.migration_pending,
        migrated: home.migrated,
        port,
        version: crate::server::probe_health(port, Duration::from_secs(2)),
        identity: crate::bus_auth::app_identity::identity(),
    }
}

pub fn status(paths: &ServicePaths, configured_port: u16) -> bool {
    let port = crate::home::runtime_port(&paths.home).unwrap_or(configured_port);
    let installed = paths.bin_path().is_file() && paths.plist_path().is_file();
    let version = crate::server::probe_health(port, Duration::from_secs(2));
    println!(
        "task: {}; daemon: {} (127.0.0.1:{port})",
        if installed { "installed" } else { "missing" },
        version.as_deref().unwrap_or("unreachable")
    );
    println!(
        "identity: {}",
        crate::bus_auth::app_identity::identity_line()
    );
    installed && version.is_some()
}

#[cfg(test)]
mod tests;
