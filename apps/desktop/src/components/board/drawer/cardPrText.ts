// A card's pull request in its drawer (0.24.0): where it stands, in one line.

import type { PullRequest } from "../../../protocol/gen/flotilla/pr/v1/pr_pb";
import { PrState } from "../../../protocol/gen/flotilla/pr/v1/pr_pb";
import type { Tone } from "../../prs/prText";
import { baseOf, stateLine, waitingFor } from "../../prs/prText";

/** "✓ Merged into develop · 14:02", "Waiting for: you, and checks", "Everything is in: …". */
export function cardPrState(
  pr: PullRequest,
  now: number,
): { readonly text: string; readonly tone: Tone } {
  const done = stateLine(pr, now);
  if (done !== null) {
    const tone: Tone =
      pr.state === PrState.MERGED ? "ok" : pr.state === PrState.MERGING ? "you" : "dim";
    return { text: done, tone };
  }
  if (pr.mergeable?.ok === true) {
    return { text: `Everything is in: it merges into ${baseOf(pr)} next`, tone: "ok" };
  }
  const waiting = waitingFor(pr);
  return waiting === ""
    ? { text: "Open", tone: "dim" }
    : { text: `Waiting for: ${waiting}`, tone: waiting.startsWith("you") ? "you" : "warn" };
}
