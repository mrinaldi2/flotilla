// Owner actions (H-117 R1/R2): an exact command a bot proposes and only the
// owner runs, with one tap, from the app or the phone. The client must say
// it renders them (hello feature `owner_actions`) to get them at all.

/** Push requests (0.24.0): owner actions with `kind: "push"`. */
export const PUSH_REQUESTS_V1 = "push_requests_v1";

type OwnerActionState =
  | "proposed"
  | "running"
  | "succeeded"
  | "failed"
  | "timed_out"
  | "rejected"
  | "withdrawn"
  | "expired";

export interface OwnerAction {
  readonly id: string;
  readonly project_id: string;
  /** `bot:<id>`, or `daemon` for a template the daemon filled. */
  readonly proposed_by: string;
  readonly item_id: string | null;
  readonly decision_id: string | null;
  /** The target daemon's id. */
  readonly target_machine: string;
  /** "this computer", or the linked computer's name. */
  readonly target_name?: string;
  readonly shell: string;
  readonly cwd: string;
  /** Verbatim: what runs, exactly. */
  readonly content: string;
  readonly pinned_files: readonly { readonly path: string; readonly sha256: string }[];
  readonly reason: string;
  readonly timeout_s: number;
  /** What the client shows and sends back to run it. */
  readonly sha256: string;
  /** Warnings, e.g. a path a bot can change that isn't pinned. */
  readonly flags: readonly string[];
  readonly state: OwnerActionState;
  readonly created_at: string;
  readonly expires_at: string;
  readonly run_at?: string | null;
  readonly finished_at?: string | null;
  readonly exit_code: number | null;
  /** The redacted end of its output. */
  readonly output_tail: string | null;
  readonly reject_reason: string | null;
  /** `push` (0.24.0, `push_requests_v1`): a bot's `request_push`; absent, a command. */
  readonly kind?: "command" | "push";
  /** A push request's details, read from the bot's worktree when it asked. */
  readonly push?: PushDetails;
}

/** What a push request pushes: `git push origin <branch>:<target>`. */
interface PushDetails {
  readonly branch: string;
  readonly target: string;
  /** "owner/name". */
  readonly repo: string;
  readonly commit: string;
  readonly files: number;
  readonly additions: number;
  readonly deletions: number;
  readonly item_id?: string;
}

export type OwnerActionRequestBody =
  | { readonly type: "owner_action_list"; readonly project_id?: string }
  | { readonly type: "owner_action_get"; readonly id: string }
  /** Needs approve; `sha256` is the hash the card showed. */
  | { readonly type: "owner_action_run"; readonly id: string; readonly sha256: string }
  | { readonly type: "owner_action_reject"; readonly id: string; readonly reason?: string };

export type OwnerActionReply =
  | { readonly type: "owner_actions"; readonly actions: readonly OwnerAction[] }
  | { readonly type: "owner_action"; readonly action: OwnerAction };

/** The hello feature that asks for owner actions. */
export const OWNER_ACTIONS_FEATURE = "owner_actions";
