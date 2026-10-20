//! A bot's branch link is only reviewable work once the branch is on the
//! project's remote (0.23.1): linking a branch that was never pushed is
//! refused, so a card can't reach Review with work no one else can read.

use std::sync::Arc;

use crate::app::AppState;
use crate::checkouts::mirror::Mirror;

/// The branch a link names, without a remote's prefix (`origin/x` is `x`).
fn branch_name(reference: &str) -> &str {
    let name = reference.trim();
    let name = name.strip_prefix("refs/heads/").unwrap_or(name);
    name.strip_prefix("origin/").unwrap_or(name)
}

/// `Err` with what to do when the project has a repository and `reference`
/// isn't a branch on its remote. A project without one, or a mirror that
/// can't be fetched, isn't checked.
pub fn check_branch(app: &Arc<AppState>, project_id: &str, reference: &str) -> anyhow::Result<()> {
    if app.db.project_repo(project_id)?.is_none() {
        return Ok(());
    }
    let Ok(mirror) = Mirror::of_project(app, project_id) else {
        return Ok(());
    };
    if let Err(error) = mirror.fetch() {
        tracing::warn!(project_id, %error, "branch link: mirror fetch failed; not checked");
        return Ok(());
    }
    let branch = branch_name(reference);
    if mirror
        .resolve(&format!("refs/remotes/origin/{branch}"))
        .is_some()
    {
        return Ok(());
    }
    anyhow::bail!(
        "the branch {branch} isn't on the project's remote: push it \
         (git push origin HEAD:{branch}), then link it"
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_remote_prefix_is_not_part_of_the_branch() {
        assert_eq!(
            super::branch_name("origin/tester/tour-lock"),
            "tester/tour-lock"
        );
        assert_eq!(super::branch_name("refs/heads/a/b"), "a/b");
        assert_eq!(super::branch_name(" lead/x "), "lead/x");
    }
}
