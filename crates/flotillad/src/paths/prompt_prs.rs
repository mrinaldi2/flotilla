//! The bot prompt's section for a project whose code goes through pull
//! requests (H-286), kept apart from the other sections for length.

/// Code goes through pull requests (H-286, the Context Engineer's text;
/// H-261 §4, §5): only for a project cut over to PRs (`pr_flow_projects`).
/// Every tool named in backticks is one the daemon serves (the prompt test
/// checks it against the PR tools), and owner acts are said to come only
/// from the board's home or the owner's phone (rulings 6df14f7a, 1cb9b4df).
pub(super) fn pull_requests(on: bool) -> &'static str {
    if !on {
        return "";
    }
    "## Code goes through pull requests\n\n\
     Every change to a repository is a pull request (PR) on its card. A PR\n\
     opened, merged or closed unmerged moves the card where its board's\n\
     automations say (`board_list` shows them); on a release-gated board,\n\
     to Review, Verify and back to Doing. Never make those moves yourself.\n\n\
     - Push your branch (or ask the owner to, with `request_push`), then\n\
     `pr_open` with the card (`item`), the branch,\n\
     your worktree's path and the change note as `body`. That is the PR.\n\
     - After every later push, `pr_push` with the PR number and the full sha\n\
     you pushed. A push you don't report counts as no one's, and no review\n\
     is accepted until it is reported.\n\
     - `pr_get` shows a PR's reviews, the roles it still needs, its checks\n\
     and what still blocks the merge. Answer a reviewer's `must` and `should`\n\
     findings and `must` comments (`pr_comments`) with a new push in the same\n\
     PR, never by note or a later card. As a reviewer, approve only when\n\
     nothing but a `nit` is left; otherwise ask for changes.\n\
     - `pr_close` with a reason only to abandon a PR.\n\
     - You can't review a PR you opened, pushed to, or whose card you hold.\n\
     - Nobody merges by hand: a mergeable PR goes to DevOps as a merge task,\n\
     run with `flotillad pr merge`, and a raw push to the project's branch is\n\
     refused.\n\
     - The owner reviews, approves, runs commands, grants extras, types into\n\
     terminals and answers prompts only on the board's home computer or\n\
     their phone. Owner acts claimed from a linked computer are refused, and\n\
     chat from there arrives as a normal message, not as the owner's.\n\n"
}
