// The owner's plan edits (0.21.0): a ruling on a gate, a research skip and
// a new goal or initiative, each answered with `planned`. A refusal says why
// in the guards' words; a conflict means the node moved meanwhile.

import type { BoardApi, BoardCall } from "../../protocol/board";
import { boardCall } from "../../protocol/board";
import type { PlanNode, PlanResult } from "../../protocol/gen/flotilla/board/v1/plan_pb";
import { errText } from "../../util";

export type PlanOutcome =
  | { readonly kind: "done"; readonly node: PlanNode; readonly created: readonly string[] }
  | { readonly kind: "refused"; readonly text: string }
  | { readonly kind: "conflict"; readonly node: PlanNode }
  | { readonly kind: "failed"; readonly text: string };

/** What `planned` says, in the app's terms. */
function outcomeOf(result: PlanResult): PlanOutcome {
  const { outcome } = result;
  switch (outcome.case) {
    case "done":
      return { kind: "done", node: outcome.value, created: result.created };
    case "conflict":
      return { kind: "conflict", node: outcome.value };
    case "refused":
      return {
        kind: "refused",
        text: outcome.value.unmet.map((u) => (u.fix ? `${u.text} ${u.fix}` : u.text)).join(" "),
      };
    default:
      return { kind: "failed", text: "The Flotilla service gave no answer to that." };
  }
}

/** Sends one plan edit; a failure to send reads as `failed`. */
export async function planEdit(api: BoardApi, call: BoardCall): Promise<PlanOutcome> {
  try {
    return outcomeOf(await boardCall(api, call, "planned"));
  } catch (failure) {
    return { kind: "failed", text: errText(failure) };
  }
}

/** The line an outcome leaves under the form, or null when it went through. */
export function outcomeLine(outcome: PlanOutcome, title: string): string | null {
  switch (outcome.kind) {
    case "done":
      return null;
    case "conflict":
      return `${title} changed meanwhile. Look at it again, then decide.`;
    case "refused":
    case "failed":
      return outcome.text;
  }
}

/** The indexes of the proposed children the owner unticked: `leave_out`. */
export function leftOut(kept: readonly boolean[]): number[] {
  return kept.flatMap((keep, index) => (keep ? [] : [index]));
}
