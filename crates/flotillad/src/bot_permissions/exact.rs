//! This daemon's own binary in permission rules and commands (H-166).
//!
//! Bots have no `flotillad` on their PATH: they run the daemon by its full
//! path, which on a Mac has a space (`/Applications/Flotilla.app/…`). A
//! rule for a bare `flotillad …` never matches that, so auto mode sent
//! `release install` to the classifier, which refused it. A bare name would
//! also be too wide: it allows whatever `flotillad` the bot's PATH finds
//! first. So a rule names this binary by its exact path, in each way a bot
//! may quote it, and only the subcommand's arguments are a wildcard.

use std::path::{Path, PathBuf};

/// `Bash(<this binary> <sub> *)` for every spelling of the path, plus the
/// PowerShell forms on Windows. None when the path itself holds a character
/// a rule would read as a pattern.
pub(super) fn rules(flotillad: &Path, sub: &str) -> Vec<String> {
    let path = flotillad.display().to_string();
    if path.is_empty() || path.contains(['*', '(', ')', '"', '\'']) {
        return Vec::new();
    }
    let spaced = path.contains(char::is_whitespace);
    let mut spellings = vec![format!("\"{path}\""), format!("'{path}'")];
    if !spaced {
        spellings.insert(0, path.clone());
    } else if !cfg!(windows) {
        spellings.push(path.replace(' ', "\\ "));
    }
    let mut rules: Vec<String> = spellings
        .iter()
        .map(|p| format!("Bash({p} {sub} *)"))
        .collect();
    if cfg!(windows) {
        // Git Bash spells it with `/`; PowerShell calls a quoted path with `&`.
        let slashed = path.replace('\\', "/");
        rules.push(format!("Bash(\"{slashed}\" {sub} *)"));
        rules.extend(
            spellings
                .iter()
                .map(|p| format!("PowerShell(& {p} {sub} *)")),
        );
    }
    rules
}

/// Whether a bot could rebuild or replace the binary at `flotillad` (CE-023
/// F2): a cargo target (a dev daemon), anything under `~/Developer`, the
/// trusted paths, the bots' own folders, `~/.cargo` or a temp dir. A rule
/// naming such a binary would pre-approve whatever a bot puts there, so it
/// gets none and the call goes to the classifier.
pub(super) fn bot_writable(
    flotillad: &Path,
    home: &Path,
    user_home: &Path,
    trusted: &[PathBuf],
) -> bool {
    let cargo_target = flotillad.ancestors().skip(1).any(|dir| {
        dir.file_name().is_some_and(|n| n == "target")
            || dir.join(".rustc_info.json").exists()
            || dir.join("CACHEDIR.TAG").exists()
    });
    let mut roots: Vec<PathBuf> = trusted.to_vec();
    roots.extend([
        user_home.join("Developer"),
        user_home.join(".cargo"),
        home.join("projects"),
        std::env::temp_dir(),
    ]);
    roots.extend(
        [
            "/tmp",
            "/private/tmp",
            "/var/folders",
            "/private/var/folders",
        ]
        .into_iter()
        .map(PathBuf::from),
    );
    let real = flotillad.canonicalize().ok();
    cargo_target
        || roots.iter().any(|root| {
            let real_root = root.canonicalize().ok();
            [Some(flotillad), real.as_deref()]
                .into_iter()
                .flatten()
                .any(|p| {
                    p.starts_with(root) || real_root.as_ref().is_some_and(|r| p.starts_with(r))
                })
        })
}

/// The command line a bot runs, in the first spelling `rules` allows:
/// `"/Applications/Flotilla.app/Contents/MacOS/flotillad" release install <id>`.
pub fn command(flotillad: &Path, args: &str) -> String {
    let path = flotillad.display().to_string();
    if path.contains(char::is_whitespace) {
        format!("\"{path}\" {args}")
    } else {
        format!("{path} {args}")
    }
}

/// This daemon's binary, as `rules` and `command` take it.
pub fn this_binary() -> PathBuf {
    std::env::current_exe().unwrap_or_else(|_| "flotillad".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(windows))]
    const APP: &str = "/Applications/Flotilla Beta.app/Contents/MacOS/flotillad";

    #[cfg(not(windows))]
    #[test]
    fn a_path_with_a_space_is_allowed_in_each_quoting_only() {
        let rules = rules(Path::new(APP), "release install");
        assert_eq!(
            rules,
            [
                format!("Bash(\"{APP}\" release install *)"),
                format!("Bash('{APP}' release install *)"),
                format!("Bash({} release install *)", APP.replace(' ', "\\ ")),
            ]
        );
        // The command a bot is told to run matches the first rule.
        let line = command(Path::new(APP), "release install");
        assert_eq!(line, format!("\"{APP}\" release install"));
        assert_eq!(rules[0], format!("Bash({line} *)"));
    }

    #[cfg(not(windows))]
    #[test]
    fn a_plain_path_is_allowed_bare_and_quoted() {
        let rules = rules(Path::new("/usr/local/bin/flotillad"), "quiesce");
        assert_eq!(rules[0], "Bash(/usr/local/bin/flotillad quiesce *)");
        assert_eq!(rules.len(), 3);
        assert!(!rules.iter().any(|r| r.starts_with("Bash(flotillad")));
    }

    #[test]
    fn a_path_a_rule_would_read_as_a_pattern_gets_no_rule() {
        for odd in [
            "/opt/*/flotillad",
            "/opt/a(b)/flotillad",
            "/opt/it's/flotillad",
            "",
        ] {
            assert!(rules(Path::new(odd), "quiesce").is_empty(), "{odd}");
        }
    }

    #[cfg(not(windows))]
    #[test]
    fn a_binary_a_bot_could_rebuild_is_bot_writable() {
        let (home, user) = (Path::new("/Users/me/.flotilla"), Path::new("/Users/me"));
        let trusted = [PathBuf::from("/Users/me/Code")];
        for dev in [
            "/Users/me/Developer/gravity/target/debug/flotillad",
            "/Users/me/Developer/flotillad",
            "/Users/me/elsewhere/target/release/flotillad",
            "/Users/me/Code/gravity/out/flotillad",
            "/Users/me/.cargo/bin/flotillad",
            "/Users/me/.flotilla/projects/p/bots/dev/workspace/flotillad",
            "/tmp/flotillad",
        ] {
            assert!(bot_writable(Path::new(dev), home, user, &trusted), "{dev}");
        }
        assert!(!bot_writable(Path::new(APP), home, user, &trusted));
    }
}
