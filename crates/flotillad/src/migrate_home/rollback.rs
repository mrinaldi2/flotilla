//! `migrate-home --rollback`: undoes a run from its action log.

use std::io::Write;
use std::path::Path;
use std::time::Duration;

use anyhow::{bail, Context};

use super::{daemon_stopped, disk, files, sql, steps, Action, Plan, State, Step, STATE_FILE};

fn is_real_dir(path: &std::path::Path) -> bool {
    path.symlink_metadata()
        .is_ok_and(|meta| meta.is_dir() && !meta.file_type().is_symlink())
}

fn links_to(link: &Path, target: &Path) -> bool {
    link.symlink_metadata()
        .is_ok_and(|meta| meta.file_type().is_symlink())
        && link.canonicalize().ok() == target.canonicalize().ok()
}

/// What would stop the rollback partway: a renamed path whose old name is
/// taken again (a The Hermes daemon started through the link writes
/// `hermesd.toml`; a session under the old path makes its transcript dir).
/// Checked before anything is undone, so a refused rollback changes nothing.
fn in_the_way(plan: &Plan, state: &State) -> Vec<String> {
    let linked = state
        .actions
        .iter()
        .any(|a| matches!(a, Action::Symlink { .. }));
    let mut blocked = Vec::new();
    for action in &state.actions {
        let Action::Rename { from, to } = action else {
            continue;
        };
        let Ok(meta) = from.symlink_metadata() else {
            continue;
        };
        let clear = if from == &plan.from {
            // The link is removed first; a recreated directory is set aside.
            (meta.file_type().is_symlink() && (linked || links_to(from, to))) || is_real_dir(from)
        } else {
            to.symlink_metadata().is_err()
        };
        if !clear {
            blocked.push(from.display().to_string());
        }
    }
    blocked
}

/// Reverses a migration from its log, newest change first, and leaves the
/// old home exactly where it was. The daemon must be stopped; after the new
/// service has run, uninstall it first (`flotillad service uninstall`).
pub fn rollback(plan: &Plan, out: &mut dyn Write) -> anyhow::Result<()> {
    let Some(mut state) = plan.state()? else {
        bail!("no migration to roll back (no {STATE_FILE})");
    };
    let home = if state.done(Step::Move) {
        &plan.to
    } else {
        &plan.from
    };
    daemon_stopped(home, Duration::from_secs(10)).context("stop the daemon first")?;
    let blocked = in_the_way(plan, &state);
    if !blocked.is_empty() {
        bail!(
            "nothing was undone: {} would have to be overwritten to put the old home \
             back; move it aside and run `flotillad migrate-home --rollback` again",
            blocked.join(", ")
        );
    }
    // From here the run is partly undone: neither finished nor resumable.
    if !state.rolling_back || state.completed_at.is_some() {
        state.rolling_back = true;
        state.completed_at = None;
        state.save()?;
    }
    while let Some(action) = state.actions.pop() {
        writeln!(out, "undo {action:?}")?;
        match &action {
            Action::Symlink { path } => steps::remove_link(path)?,
            Action::File { path, original } => files::write(path, original)?,
            Action::Database { previous, .. } => {
                sql::apply(
                    &plan.to.join("bus.sqlite"),
                    &plan.reversed(),
                    sql::Mark::Restore(previous.as_ref()),
                )?;
            }
            // A file the new daemon removed since (its port file goes at
            // every stop, `service uninstall` included) has nothing to move
            // back. The home itself is never treated so.
            Action::Rename { from, to } if from != &plan.from && to.symlink_metadata().is_err() => {
                writeln!(out, "{} is gone; nothing to move back", to.display())?;
            }
            Action::Rename { from, to } => {
                // Recreated by a process that kept writing to the old path
                // (the Symlink step refuses to run over it): set aside,
                // never merged into or deleted.
                // Linked by a run killed before it could log the link.
                if from == &plan.from && links_to(from, to) {
                    steps::remove_link(from)?;
                }
                if from == &plan.from && is_real_dir(from) {
                    let aside = disk::set_aside(from)?;
                    writeln!(
                        out,
                        "{} was recreated after the move; moved it to {}, merge it by hand",
                        from.display(),
                        aside.display()
                    )?;
                }
                if from.symlink_metadata().is_ok() {
                    bail!(
                        "cannot move {} back: {} exists",
                        to.display(),
                        from.display()
                    );
                }
                // A merged transcript dir was emptied and removed.
                if let Some(parent) = from.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::rename(to, from).with_context(|| {
                    format!("moving {} back to {}", to.display(), from.display())
                })?;
                if from == &plan.from {
                    state.steps.retain(|s| *s != Step::Move);
                }
            }
        }
        state.save()?;
        super::crash_point("undo")?;
    }
    std::fs::remove_file(plan.from.join(STATE_FILE))?;
    super::hop::restore_previous(plan, state.previous.as_deref())?;
    writeln!(out, "rolled back; {} is as it was", plan.from.display())?;
    Ok(())
}
