// What the lead changed on a board (0.19.0): the settings panel's footer
// and the notice strip over the board read the lead's applied changes from
// the snapshot, newest first.

import { timestampDate } from "@bufbuild/protobuf/wkt";
import type { BoardProposal } from "../../../protocol/gen/hermes/board/v1/requests_pb";
import { age } from "../../control/decisions";

function at(proposal: BoardProposal): string {
  return proposal.at === undefined ? "" : timestampDate(proposal.at).toISOString();
}

/** The lead's applied changes to `boardId`, newest first. */
export function leadChanges(proposals: readonly BoardProposal[], boardId: string): BoardProposal[] {
  return proposals.filter((p) => p.state === "applied" && p.boardId === boardId);
}

/** The lead's open proposals for `boardId` (or a new board, when unset). */
export function openProposals(proposals: readonly BoardProposal[]): BoardProposal[] {
  return proposals.filter((p) => p.state === "open");
}

/** "Last change by Lead, 3h ago: add column “QA”", or the board's own words. */
export function lastChangeLine(
  proposals: readonly BoardProposal[],
  boardId: string,
  who: (actor: string) => string,
  now: number,
): string {
  const last = leadChanges(proposals, boardId)[0];
  if (last === undefined) {
    return "Every save is a change to the board, which the lead is told about.";
  }
  return `Last change by ${who(last.proposedBy)}, ${age(at(last), now)}: ${last.summary}`;
}

/** "↻ Lead: add column “QA” · 3h ago" for the notice strip. */
export function noticeLine(
  change: BoardProposal,
  who: (actor: string) => string,
  now: number,
): string {
  return `${who(change.proposedBy)}: ${change.summary} · ${age(at(change), now)}`;
}
