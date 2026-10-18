// What the lead asks of the owner besides a document (0.21.0): an
// initiative it proposes under a goal, and its request to skip an
// initiative's research. Each waits as a gate: PROPOSED, or RESEARCH with
// the lead's note.

import { timestampFromDate } from "@bufbuild/protobuf/wkt";
import type { PlanNode, PlanTree } from "../protocol/gen/flotilla/board/v1/plan_pb";
import { PlanKind, PlanResearchState, PlanStage } from "../protocol/gen/flotilla/board/v1/plan_pb";
import { node, planNodes, planTree } from "./planFixtures";

const at = (iso: string) => timestampFromDate(new Date(iso));

/** PL-12: Boss fights, proposed by the lead under the goal. */
function proposedInitiative(): PlanNode {
  return node({
    id: "PL-12",
    kind: PlanKind.INITIATIVE,
    parentId: "PL-1",
    title: "Boss fights",
    summary: "Bosses are the playtests' top complaint: they read as damage sponges.",
    stage: PlanStage.PROPOSED,
    rank: "a3",
    gate: {
      stage: PlanStage.PROPOSED,
      submittedBy: "bot:lead",
      submittedAt: at("2026-10-17T09:00:00Z"),
      note: "Six of ten playtesters named a boss fight as the low point.",
      round: 1,
    },
  });
}

/** PL-9: Combat onboarding, its lead asking to skip research. */
function skipRequest(): PlanNode {
  return node({
    id: "PL-9",
    kind: PlanKind.INITIATIVE,
    parentId: "PL-1",
    title: "Combat onboarding",
    summary: "New players learn to block and dodge.",
    stage: PlanStage.RESEARCH,
    boardId: "p1/onboarding",
    rank: "a2",
    research: { state: PlanResearchState.OPEN, spikes: [] },
    gate: {
      stage: PlanStage.RESEARCH,
      submittedBy: "bot:lead",
      submittedAt: at("2026-10-17T11:00:00Z"),
      note: "The tutorial study from 0.18 already answers it.",
      round: 1,
    },
  });
}

/** The plan with both requests waiting, beside the HLD gate. */
export function requestsTree(): PlanTree {
  const nodes = planNodes().filter((n) => n.id !== "PL-9");
  return planTree({ nodes: [...nodes, skipRequest(), proposedInitiative()] });
}
