//! The daemon's own commands a bot runs pre-approved (`release install`,
//! `quiesce`, …; H-166) talk to the daemon its config names. Pointed at
//! another config or home, they could ask a fake daemon a bot runs, which
//! approves anything (CE-023 F1). So from a bot they run as told: no
//! `--config`, no home override, no changed environment.

use super::paths::Scope;
use crate::bot_permissions::shell::{self, Words};

/// `release <sub>`, `quiesce` and `pr merge` (H-284): the subcommands an
/// extra pre-approves.
const RELEASE: [&str; 6] = [
    "install",
    "publish",
    "land",
    "tag",
    "leave-out",
    "build-installer",
];

/// Why this `flotillad` line must not run, or `None`.
pub(super) fn redirected(words: &Words, at: usize, scope: &Scope) -> Option<String> {
    // From the program on: `env -u NAME flotillad …` names NAME as the program.
    let is_daemon = |w: &String| {
        let name = shell::program(w);
        matches!(
            name.strip_suffix(".exe").unwrap_or(name),
            "flotillad" | "hermesd" | "gravityd"
        )
    };
    let at = if is_daemon(&words[at]) {
        at
    } else if words[..at].iter().any(|w| shell::program(w) == "env") {
        at + words[at..].iter().position(is_daemon)?
    } else {
        return None;
    };
    let rest = &words[at + 1..];
    // `flotillad --config X release install …`: main reads the flag anywhere.
    let mut i = 0;
    while rest.get(i).is_some_and(|w| w.starts_with('-')) {
        i += if rest[i] == "--config" { 2 } else { 1 };
    }
    let sub = rest.get(i..);
    let gated = match sub {
        Some([first, ..]) if first == "quiesce" => true,
        Some([first, second, ..]) if first == "pr" => second == "merge",
        Some([first, second, ..]) if first == "release" => RELEASE.contains(&second.as_str()),
        _ => false,
    };
    if !gated {
        return None;
    }
    let config = rest
        .iter()
        .any(|w| w == "--config" || w.starts_with("--config="));
    let mut homes = vec!["HOME".to_string()];
    homes.extend(crate::brand::all_env_names("HOME"));
    // `X=… flotillad`, `env -u … flotillad`: its environment, so its home, is changed.
    let prefixed = words[..at]
        .iter()
        .any(|w| shell::is_assignment(w) || shell::program(w) == "env");
    let home = prefixed || scope.vars.keys().any(|k| homes.contains(k));
    (config || home).then(|| {
        "run the daemon's command exactly as given, without `--config`, a home override \
         or a changed environment: it must reach this computer's daemon"
            .to_string()
    })
}
