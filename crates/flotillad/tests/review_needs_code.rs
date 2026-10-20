//! A bot's code card reaches Review only with its work where others can
//! read it (0.23.1): linking a branch checks it is on the project's remote.

use crate::common;
use common::plan::*;

#[tokio::test]
async fn only_a_branch_on_the_remote_counts_as_pushed() {
    let plan = unplanned().await;
    let app = &plan.d.app;
    flotillad::kanban::pushed::check_branch(app, &plan.pid, "main").expect("main is pushed");
    flotillad::kanban::pushed::check_branch(app, &plan.pid, "origin/main")
        .expect("a remote prefix is fine");
    let refused = flotillad::kanban::pushed::check_branch(app, &plan.pid, "tester/never-pushed")
        .expect_err("a local branch isn't pushed");
    assert!(
        refused
            .to_string()
            .contains("git push origin HEAD:tester/never-pushed"),
        "{refused}"
    );
    // A project without a repository isn't checked.
    app.db
        .set_project_repo(&plan.pid, None)
        .expect("clear repo");
    flotillad::kanban::pushed::check_branch(app, &plan.pid, "tester/never-pushed")
        .expect("no repository, nothing to check");
}
