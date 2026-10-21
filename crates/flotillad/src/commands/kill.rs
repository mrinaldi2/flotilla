//! Ending a background command's processes (0.24.0): the runtime writes a
//! background command's output to its own file (`…/tasks/<id>.output`), so
//! the processes holding that file open are the command's. Each one at the
//! top of that set is ended with its process group when it leads one, else
//! with the processes under it. The bot's own session, and anything above
//! it, is never touched: a runtime that keeps the file open itself is not
//! the command.

use std::path::Path;

/// How many processes were signalled; an error where this computer can't
/// tell a command's processes.
pub fn end(output_file: &Path, keep: &[u32]) -> anyhow::Result<usize> {
    imp::end(output_file, keep)
}

#[cfg(unix)]
mod imp {
    use std::collections::HashSet;
    use std::path::Path;

    use crate::holders::procs::{self, Row};

    /// The processes with `path` open.
    #[cfg(target_os = "macos")]
    fn holders(path: &Path) -> Vec<u32> {
        let Some(lsof) = ["/usr/sbin/lsof", "/usr/bin/lsof"]
            .into_iter()
            .find(|p| Path::new(p).is_file())
        else {
            return Vec::new();
        };
        let Ok(out) = std::process::Command::new(lsof)
            .args(["-w", "-t", "--"])
            .arg(path)
            .output()
        else {
            return Vec::new();
        };
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter_map(|l| l.trim().parse().ok())
            .collect()
    }

    /// The processes with `path` open, read from `/proc`.
    #[cfg(not(target_os = "macos"))]
    fn holders(path: &Path) -> Vec<u32> {
        let Ok(target) = std::fs::canonicalize(path) else {
            return Vec::new();
        };
        let Ok(entries) = std::fs::read_dir("/proc") else {
            return Vec::new();
        };
        entries
            .flatten()
            .filter_map(|e| e.file_name().to_str()?.parse::<u32>().ok())
            .filter(|pid| {
                std::fs::read_dir(format!("/proc/{pid}/fd")).is_ok_and(|fds| {
                    fds.flatten()
                        .any(|fd| std::fs::read_link(fd.path()).is_ok_and(|p| p == target))
                })
            })
            .collect()
    }

    /// `pid` and every process above it.
    fn ancestors(rows: &[Row], pid: u32) -> HashSet<u32> {
        let mut out = HashSet::new();
        let mut at = Some(pid);
        while let Some(pid) = at.filter(|p| *p > 1 && out.insert(*p)) {
            at = rows.iter().find(|r| r.pid == pid).map(|r| r.ppid);
        }
        out
    }

    /// `pid` and every process under it.
    fn tree(rows: &[Row], pid: u32) -> Vec<u32> {
        let mut out = vec![pid];
        let mut i = 0;
        while i < out.len() {
            let parent = out[i];
            out.extend(rows.iter().filter(|r| r.ppid == parent).map(|r| r.pid));
            i += 1;
        }
        out
    }

    fn signal(pid: u32, group: bool, sig: libc::c_int) {
        let Ok(pid) = libc::pid_t::try_from(pid) else {
            return;
        };
        // SAFETY: plain signal delivery to a process or its group.
        unsafe {
            if group {
                libc::killpg(pid, sig);
            } else {
                libc::kill(pid, sig);
            }
        }
    }

    pub fn end(path: &Path, keep: &[u32]) -> anyhow::Result<usize> {
        let held: HashSet<u32> = holders(path).into_iter().collect();
        if held.is_empty() {
            return Ok(0);
        }
        let rows = procs::all();
        // The daemon, the bot's session and everything above them.
        let mut kept: HashSet<u32> = HashSet::new();
        for pid in keep.iter().copied().chain([std::process::id()]) {
            kept.extend(ancestors(&rows, pid));
        }
        let mut targets: Vec<(u32, bool)> = Vec::new();
        for &pid in &held {
            let parent = rows.iter().find(|r| r.pid == pid).map(|r| r.ppid);
            if kept.contains(&pid) || parent.is_some_and(|p| held.contains(&p)) {
                continue;
            }
            let leads = rows
                .iter()
                .find(|r| r.pid == pid)
                .is_some_and(|r| r.pgid == Some(pid));
            // A group leader's group holds nothing kept, or it isn't ended
            // as a group.
            let group_clear = leads
                && !rows
                    .iter()
                    .any(|r| r.pgid == Some(pid) && kept.contains(&r.pid));
            if group_clear {
                targets.push((pid, true));
            } else {
                targets.extend(
                    tree(&rows, pid)
                        .into_iter()
                        .filter(|p| !kept.contains(p))
                        .map(|p| (p, false)),
                );
            }
        }
        for &(pid, group) in &targets {
            signal(pid, group, libc::SIGTERM);
        }
        // Whatever ignores the polite signal ends a moment later, unless
        // its pid now names another process.
        let late: Vec<(u32, bool, Option<u64>)> = targets
            .iter()
            .map(|&(pid, group)| (pid, group, procs::start_of(pid)))
            .collect();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(2));
            for (pid, group, start) in late {
                if start.is_some() && procs::start_of(pid) == start {
                    signal(pid, group, libc::SIGKILL);
                }
            }
        });
        Ok(targets.len())
    }
}

#[cfg(not(unix))]
mod imp {
    use std::path::Path;

    pub fn end(_path: &Path, _keep: &[u32]) -> anyhow::Result<usize> {
        Err(crate::decisions::invalid(
            "stopping a bot's command isn't supported on Windows yet",
        ))
    }
}
