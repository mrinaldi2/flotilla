//! More than one rename: Gravity (`~/.gravity`) became The Hermes
//! (`~/.thehermes`), which became Flotilla (`~/.flotilla`). Each move leaves
//! its finished `migrate-home.json` in the home it moved to, so the next
//! move finds an earlier one's log in the home it is about to move. A log
//! belongs to one move only (its `from` and `to`); any other is history.

use std::path::{Path, PathBuf};

use super::state::{State, STATE_FILE};
use super::Plan;
use crate::config::Config;

/// The default move for `cfg`: from the newest pre-rename home that has a
/// run of its own (finished or not), else the newest that holds a daemon,
/// else from `~/.thehermes`, which then has nothing to do.
pub(super) fn default_plan(cfg: &Config) -> Plan {
    let plans: Vec<Plan> = crate::brand::LEGACY_HOME_DIR_NAMES
        .iter()
        .map(|name| {
            Plan::new(
                cfg.user_home.join(name),
                cfg.home.clone(),
                cfg.user_home.clone(),
            )
        })
        .collect();
    plans
        .iter()
        .find(|plan| matches!(plan.state(), Ok(Some(_))))
        .or_else(|| plans.iter().find(|plan| plan.source_is_real_home()))
        .unwrap_or(&plans[0])
        .clone()
}

/// The log of `plan`'s own run, if there is one: in the destination once the
/// home has moved, in the source before that. A log of another move (the
/// previous rename's, in the home this one moves) is not it.
pub(super) fn locate(plan: &Plan) -> anyhow::Result<Option<(PathBuf, State)>> {
    for home in [&plan.to, &plan.from] {
        let path = home.join(STATE_FILE);
        if !path.is_file() {
            continue;
        }
        let state = State::load(&path)?;
        if same_home(&state.from, &plan.from) && same_home(&state.to, &plan.to) {
            return Ok(Some((path, state)));
        }
    }
    Ok(None)
}

/// Whether two spellings name the same home: as given, or with the parent
/// resolved through links (macOS `/var` is `/private/var`), ignoring case on
/// Windows and macOS.
pub(super) fn same_home(a: &Path, b: &Path) -> bool {
    let resolve = |path: &Path| -> PathBuf {
        path.parent()
            .and_then(|parent| parent.canonicalize().ok())
            .zip(path.file_name())
            .map_or_else(|| path.to_path_buf(), |(parent, name)| parent.join(name))
    };
    let key = |path: PathBuf| -> String {
        let text = path.to_string_lossy().replace('\\', "/");
        if cfg!(any(windows, target_os = "macos")) {
            text.to_lowercase()
        } else {
            text
        }
    };
    a == b || key(resolve(a)) == key(resolve(b))
}

/// Sets aside an earlier move's log found in the home `plan` is about to
/// move, under a name of its own beside it, and records that name, so this
/// run's log can take its place and a rollback can put it back.
pub(super) fn keep_previous(plan: &Plan, state: &mut State) -> anyhow::Result<()> {
    let path = plan.from.join(STATE_FILE);
    if state.previous.is_some() || !path.is_file() {
        return Ok(());
    }
    let name = archived_name(&path);
    let target = (0..)
        .map(|n| match n {
            0 => plan.from.join(&name),
            n => plan
                .from
                .join(format!("{}-{n}.json", name.trim_end_matches(".json"))),
        })
        .find(|target| target.symlink_metadata().is_err())
        .unwrap_or_else(|| plan.from.join(&name));
    std::fs::rename(&path, &target)?;
    state.previous = target.file_name().map(|n| n.to_string_lossy().into_owned());
    Ok(())
}

/// `migrate-home.gravity-to-thehermes.json` for the log of that move.
fn archived_name(path: &Path) -> String {
    let stem = |p: &Path| -> Option<String> {
        Some(
            p.file_name()?
                .to_string_lossy()
                .trim_start_matches('.')
                .to_string(),
        )
    };
    State::load(path)
        .ok()
        .and_then(|old| {
            Some(format!(
                "migrate-home.{}-to-{}.json",
                stem(&old.from)?,
                stem(&old.to)?
            ))
        })
        .unwrap_or_else(|| "migrate-home.previous.json".to_string())
}

/// After a rollback removed this run's log: the earlier move's, back where
/// it was.
pub(super) fn restore_previous(plan: &Plan, previous: Option<&str>) -> anyhow::Result<()> {
    if let Some(name) = previous {
        let archived = plan.from.join(name);
        if archived.is_file() {
            std::fs::rename(&archived, plan.from.join(STATE_FILE))?;
        }
    }
    Ok(())
}
