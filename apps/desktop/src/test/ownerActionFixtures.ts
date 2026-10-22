import type { OwnerAction } from "../protocol/ownerActions";

/** A command a bot proposed for the owner, waiting to run. */
export function ownerAction(over: Partial<OwnerAction> = {}): OwnerAction {
  return {
    id: "oa-1",
    project_id: "p1",
    proposed_by: "bot:ops",
    item_id: "H-117",
    decision_id: null,
    target_machine: "d-mac",
    target_name: "this computer",
    shell: "zsh",
    cwd: "/Users/~/Developer/gravity",
    content: "brew services stop colima",
    pinned_files: [],
    reason: "Colima holds the old Flotilla service program open",
    timeout_s: 600,
    sha256: "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08",
    flags: [],
    state: "proposed",
    created_at: "2026-10-05T12:00:00Z",
    expires_at: "2026-10-06T12:00:00Z",
    exit_code: null,
    output_tail: null,
    reject_reason: null,
    ...over,
  };
}

/** When a push request's card is looked at: an hour after it was asked. */
export const PUSH_NOW = Date.parse("2025-03-04T15:02:00Z");

/** A bot's push request (0.24.0), waiting: its branch to the same name on origin. */
export function pushAction(over: Partial<OwnerAction> = {}): OwnerAction {
  return ownerAction({
    id: "oa-push",
    proposed_by: "bot:tester",
    item_id: "CASS-053",
    cwd: "/Users/~/Developer/cass-wt-tour-lock",
    content: "git push origin automation-tester/tour-lock-stack:automation-tester/tour-lock-stack",
    reason: "Ready for review; self-test 20/20.",
    timeout_s: 120,
    sha256: "a1f3c2d4e5b6978877665544332211ffeeddccbbaa99887766554433221100aa",
    created_at: "2025-03-04T14:02:00Z",
    expires_at: "2025-03-05T14:02:00Z",
    kind: "push",
    push: {
      branch: "automation-tester/tour-lock-stack",
      target: "automation-tester/tour-lock-stack",
      repo: "acme/cass",
      commit: "a334dd34c0ffee5e1d2b3a4c5d6e7f8091a2b3c4",
      files: 2,
      additions: 208,
      deletions: 3,
      item_id: "CASS-053",
    },
    ...over,
  });
}
