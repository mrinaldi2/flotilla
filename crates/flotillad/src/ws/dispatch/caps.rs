//! The capability each request needs: the lists of requests that only read
//! and of those that need `approve`; everything else needs `control`.

use bus::Capability;

/// Requests that only read. Everything else requires `control`.
const READ_ONLY: &[&str] = &[
    "review_settings_get",
    "disk_report",
    "list_projects",
    "list_bots",
    "list_messages",
    "list_conversations",
    "list_routines",
    "list_routine_runs",
    "list_deliveries",
    "list_devices",
    "search",
    "diagnostics",
    "get_config",
    "attach",
    "detach",
    "list_bot_revisions",
    "list_bot_activity",
    "list_decisions",
    "get_decision",
    "list_tags",
    "count_pending_decisions",
    "list_peers",
    "list_peer_bots",
    "list_peer_projects",
    "watch_browser",
    "unwatch_browser",
    "list_browser_activity",
    "list_bot_commands",
    // Every bot's commands, for the Commands page (0.24.0).
    "list_commands",
    "list_agent_conversations",
    "list_agent_conversation",
    "list_chat",
    "get_chat_step",
    "get_chat_image",
    "list_artifacts",
    "read_file",
    "list_permissions",
    "list_tasks",
    "get_task",
    "list_workers",
    "team_status",
    "list_releases",
    "release_machines",
    "get_release",
    "dashboard_get",
    "projects_overview",
    "item_cards_get",
    "attention_rows",
    // Reading an owner thread marks it read: the owner's own state (D6).
    "owner_threads",
    "owner_thread_get",
    "owner_thread_read",
    "quiesce_status",
    "bot_grants",
    "owner_action_list",
    "owner_action_get",
    "meeting_list",
    "meeting_get",
    "metrics_get",
    // What an iOS package offers a phone, and a device's waiting offer (H-229).
    "release_install",
    "install_offers",
    // Usage and the price table (0.24.0, `usage_v1`).
    "usage_get",
    "usage_prices",
];

/// Requests that exercise the owner's ruling authority.
///
/// `control` is running the fleet; this is answering for the owner, and the
/// registry is only worth anything if those are different grants. Withdrawing
/// and commenting stay under `control` because a bot may do both.
const APPROVE_ONLY: &[&str] = &[
    "answer_decision",
    "unanswer_decision",
    "hold_decision",
    "resume_decision",
    "confirm_decision",
    "confirm_relayed",
    "reopen_decision",
    "update_decision",
    "delete_decision",
    "publish_decisions",
    // How much bots may do without asking is the owner's call (H-031).
    "set_project_permission_profile",
    "set_bot_permission_extras",
    // The only way a release decision is settled (H-020 §2.2).
    "release_rule",
    "release_hold",
    "release_unhold",
    // Which computers a release must pass on before submit (H-115).
    "release_machines_set",
    // The one-shot backlog import is the owner's (H-020 §1.5, B6).
    "board_import",
    // Ending a pause for an install early is the owner's call (H-117).
    "quiesce_resume",
    // Running or rejecting a command proposed for the owner (H-117 R1).
    "owner_action_run",
    "owner_action_reject",
    // Closing an owner question for the owner (H-128 R2.2).
    "attention_dismiss",
    // Keeping or removing the owner's loose artifacts (0.22.0).
    "artifact_trash",
    "artifact_keep",
    // Pairing a device or linking a computer mints an owner credential
    // (CE-030 N1); see `owner_auth::CREDENTIALS`.
    "create_device",
    "revoke_device",
    "create_peer_invite",
    "add_peer",
    "revoke_peer",
    "link_peer_bot",
    "link_project",
    "unlink_project",
    // Repositories PRs may name (H-266, ARCH M1); the owner's clone (0.20.0).
    "set_project_extra_repos",
    "clone_project_repo",
    // The owner's review, comments and setting (H-269).
    "review_settings_set",
    "pr_review_submit",
    "pr_comment_add",
    "pr_flag",
    "pr_merge_undo",
    "pr_comment_resolve",
    "release_leave_out",
    "cleanup_resolve",
    // The disk a project may use (0.23.0): the owner's to set.
    "set_project_disk_budget",
    "set_project_pr_flow",
    // A project's weekly token budget, Extend today and the price table
    // (0.24.0, `usage_v1`): the owner's spending to set.
    "set_project_budget",
    "budget_extend",
    "set_usage_prices",
    // The owner's Stop on a bot's command (0.24.0).
    "stop_command",
];

/// Capability required for each request type.
pub(super) fn required_cap(kind: &str) -> Capability {
    if READ_ONLY.contains(&kind) {
        Capability::Read
    } else if APPROVE_ONLY.contains(&kind) {
        Capability::Approve
    } else {
        Capability::Control
    }
}
