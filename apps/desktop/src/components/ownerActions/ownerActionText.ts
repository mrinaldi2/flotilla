// The Run card's words (UX-022): who asks for what and where, and one
// line for how it went, in every final state.

import type { OwnerAction } from "../../protocol/ownerActions";

/** "this computer", or the linked computer's name. */
export function targetOf(action: OwnerAction): string {
  return action.target_name ?? "this computer";
}

/** "▶ DevOps asks you to run a command on win-pc". */
export function titleLine(action: OwnerAction, proposer: string): string {
  return `${proposer} asks you to run a command on ${targetOf(action)}`;
}

/** "2 s", "3 min": how long a run took. */
function took(action: OwnerAction): string | null {
  if (!action.run_at || !action.finished_at) {
    return null;
  }
  const seconds = Math.max(
    0,
    Math.round((Date.parse(action.finished_at) - Date.parse(action.run_at)) / 1000),
  );
  return seconds < 60 ? `took ${seconds} s` : `took ${Math.round(seconds / 60)} min`;
}

/** "10 minutes", "45 seconds": a time limit in words. */
function limit(seconds: number): string {
  if (seconds % 60 === 0) {
    const minutes = seconds / 60;
    return minutes === 1 ? "1 minute" : `${minutes} minutes`;
  }
  return `${seconds} seconds`;
}

/** What happened to it, once it left "waiting"; null while it waits. */
export function resultLine(action: OwnerAction, proposer: string): string | null {
  const parts = (head: string): string =>
    [head, action.exit_code === null ? null : `exit ${action.exit_code}`, took(action)]
      .filter((p) => p !== null)
      .join(" · ");
  switch (action.state) {
    case "proposed":
      return null;
    case "running":
      return `◑ Running on ${targetOf(action)}…`;
    case "succeeded":
      return `${parts("✓ Done")}. ${proposer} has the output.`;
    case "failed":
      return `${parts("✗ Failed")}. ${proposer} has been told.`;
    case "timed_out":
      return `⏱ Stopped after ${limit(action.timeout_s)}, its time limit. ${proposer} has been told.`;
    case "rejected":
      return action.reject_reason
        ? `⊘ You rejected it: ${action.reject_reason}`
        : "⊘ You rejected it.";
    case "withdrawn":
      return `Withdrawn by ${proposer}.`;
    case "expired":
      return "Expired: nobody answered within 24 hours.";
    default:
      return null;
  }
}

/** The confirm sheet's body: why, as whom, and that it's final. */
export function confirmBody(action: OwnerAction, proposer: string): string {
  const reason = action.reason.trim().replace(/[.\s]+$/u, "");
  return `${proposer} asks for this because: ${reason}. It runs as you, with your permissions on ${targetOf(action)}, and can't be undone.`;
}

/** The first 8 hex of the hash, as the card labels it. */
export function fingerprint(action: OwnerAction): string {
  return action.sha256.slice(0, 8);
}

/** "expires in 23 h", "expires in 40 min"; null once it has. */
export function expiresIn(action: OwnerAction, now: number): string | null {
  const left = Date.parse(action.expires_at) - now;
  if (!Number.isFinite(left) || left <= 0) {
    return null;
  }
  const minutes = Math.floor(left / 60_000);
  return minutes < 60
    ? `expires in ${Math.max(1, minutes)} min`
    : `expires in ${Math.floor(minutes / 60)} h`;
}

/** What a push request pushes (0.24.0): its kind and details, when it is one. */
export type PushDetails = NonNullable<OwnerAction["push"]>;

/** "a334dd34 · 2 files, +208 −3". */
export function commitLine(push: PushDetails): string {
  const files = push.files === 1 ? "1 file" : `${push.files} files`;
  return `${push.commit.slice(0, 8)} · ${files}, +${push.additions} −${push.deletions}`;
}

/**
 * GitHub's compare page for the push, when `repo` is "owner/name"; else null.
 * Branch names are plain (letters, digits, `.`, `_`, `-`, `/`), so they go as written.
 */
export function diffUrl(push: PushDetails): string | null {
  if (!/^[\w.-]+\/[\w.-]+$/u.test(push.repo)) {
    return null;
  }
  return `https://github.com/${push.repo}/compare/${push.target}...${push.commit}`;
}

/** How a push went, in push words where they read better; null while it waits. */
export function pushResultLine(action: OwnerAction, proposer: string): string | null {
  switch (action.state) {
    case "running":
      return "◑ Pushing…";
    case "succeeded":
      return `✓ Pushed. ${proposer} goes on from here.`;
    case "failed":
      return `✗ The push failed${action.exit_code === null ? "" : ` · exit ${action.exit_code}`}. ${proposer} has been told.`;
    default:
      return resultLine(action, proposer);
  }
}
