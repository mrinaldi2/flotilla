// A node's history in words (0.21.0): one line per `PlanEvent`. Its kind is
// text, so a newer daemon's kinds still read, as "<who>: <note>".

import { timestampDate } from "@bufbuild/protobuf/wkt";
import type { Timestamp } from "@bufbuild/protobuf/wkt";
import type { PlanEvent } from "../../protocol/gen/flotilla/board/v1/plan_pb";
import { stageWord } from "./planText";

/** "14 Oct": the day something happened. */
export function day(at: Timestamp | undefined): string {
  if (at === undefined) {
    return "";
  }
  return timestampDate(at).toLocaleDateString("en-GB", { day: "numeric", month: "short" });
}

function noted(note: string | undefined, lead: string): string {
  return note ? `${lead}${note}` : "";
}

type Line = (e: PlanEvent, who: string, stage: string) => string;

const LINES: Readonly<Record<string, Line>> = {
  created: (e, who) => `${who} created it${noted(e.note, ": ")}`,
  proposed: (e, who) => `${who} proposed it${noted(e.note, ": ")}`,
  accepted: (_e, who) => `${who} accepted the proposal`,
  declined: (e, who) => `${who} declined the proposal${noted(e.note, ": ")}`,
  submitted: (e, who, stage) => `${who} submitted the ${stage}${noted(e.note, ": ")}`,
  approved: (_e, who, stage) => `${who} approved the ${stage}`,
  sent_back: (e, who, stage) => `${who} sent the ${stage} back${noted(e.note, ": ")}`,
  research_closed: () => "Research closed: every spike is done with a document",
  research_skipped: (e, who) => `${who} skipped research${noted(e.note, ": ")}`,
  stage: (_e, _who, stage) => `Moved to ${stage}`,
  edited: (_e, who) => `${who} edited it`,
  doc_changed: (_e, _who, stage) =>
    `The approved ${stage} changed on the default branch: the lead resubmits it`,
  dropped: (e, who) => `${who} dropped it${noted(e.note, ": ")}`,
};

/** "You sent the HLD back: Add the risks.": one history event in words. */
export function historyLine(event: PlanEvent, who: (actor: string) => string): string {
  const actor = who(event.actor);
  const stage = event.stage === undefined ? "document" : stageWord(event.stage);
  const line = LINES[event.kind];
  return line ? line(event, actor, stage) : `${actor}: ${event.note ?? event.kind}`;
}
