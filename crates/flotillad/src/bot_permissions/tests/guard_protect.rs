//! The project's own shared branch is guarded like `main` (0.19.0 K0): a
//! project on `develop` gets the same protection a project on `main` does.

use bus::PermissionProfile;
use serde_json::json;

use super::super::guard::{decide, GuardContext};
use super::guard_cases::ctx;

fn on_develop() -> GuardContext {
    GuardContext {
        protect: vec!["develop".to_string()],
        ..ctx()
    }
}

fn bash(command: &str, ctx: &GuardContext) -> Option<String> {
    decide(
        &json!({
            "tool_name": "Bash",
            "tool_input": { "command": command },
            "cwd": "/Users/me/.gravity/projects/p/bots/dev/workspace"
        }),
        ctx,
    )
}

#[test]
fn the_configured_branch_is_refused_like_main() {
    let develop = on_develop();
    for command in [
        "git push origin develop",
        "git -C repo push origin HEAD:develop",
        "git push origin feat:refs/heads/develop",
        "gh api -X PATCH repos/me/x/git/refs/heads/develop",
        "git push origin main",
    ] {
        assert!(
            bash(command, &develop).is_some(),
            "{command} was let through"
        );
    }
    let refusal = bash("git push origin HEAD:develop", &develop).expect("refused");
    assert!(refusal.contains("develop"), "{refusal}");
    for command in [
        "git push origin HEAD:gravity/ch-1-01234567",
        "git push origin develop-notes",
        "git push -u origin feat/develop",
    ] {
        assert_eq!(bash(command, &develop), None, "{command} was blocked");
    }
    // Without a configured branch only `main` is protected.
    assert_eq!(bash("git push origin develop", &ctx()), None);
}

#[test]
fn pushing_the_current_branch_is_refused_when_it_is_the_configured_one() {
    let repo = tempfile::tempdir().expect("tmp");
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .arg("-C")
            .arg(repo.path())
            .args(args)
            .output()
            .expect("git")
    };
    git(&["init", "-q", "-b", "develop"]);
    git(&[
        "-c",
        "user.name=t",
        "-c",
        "user.email=t@t",
        "commit",
        "-q",
        "--allow-empty",
        "-m",
        "x",
    ]);
    let push = |command: &str, ctx: &GuardContext| {
        decide(
            &json!({
                "tool_name": "Bash",
                "tool_input": { "command": command },
                "cwd": repo.path().display().to_string()
            }),
            ctx,
        )
    };
    assert!(push("git push", &on_develop()).is_some());
    assert!(push("git push origin HEAD", &on_develop()).is_some());
    assert_eq!(push("git push", &ctx()), None);
}

#[test]
fn the_deny_rules_name_the_configured_branch_when_they_can_quote_it() {
    let rules = |branch: Option<&str>| {
        super::rules(
            &super::input_on(PermissionProfile::Trusted, &[], false, branch),
            "deny",
        )
    };
    let develop = rules(Some("develop"));
    assert!(develop.contains(&"Bash(git push * HEAD:develop)".to_string()));
    assert!(develop.contains(&"Bash(git push * main)".to_string()));
    assert!(!rules(None).iter().any(|r| r.contains("develop")));
    // A name the rule syntax can't quote is left to the guard.
    assert!(crate::bot_permissions::rule_quotable("release/2.x_rc-1"));
    assert!(!crate::bot_permissions::rule_quotable("we(ird"));
    assert!(!crate::bot_permissions::rule_quotable("a b"));
}
