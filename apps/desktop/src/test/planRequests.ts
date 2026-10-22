// What the lead asks of the owner besides a document (0.21.0): an
// initiative it proposes under a goal, and its request to skip an
// initiative's research. Each waits as a gate: PROPOSED, or RESEARCH with
// the lead's note. And research waiting for the owner's Approve findings
// (0.24.0), the HLD gate with the questions answered before it, and a push
// request on a gate.

import { create } from "@bufbuild/protobuf";
import { timestampFromDate } from "@bufbuild/protobuf/wkt";
import { ItemType } from "../protocol/gen/flotilla/board/v1/board_pb";
import type { PlanNode, PlanTree } from "../protocol/gen/flotilla/board/v1/plan_pb";
import {
  PlanDocContentSchema,
  PlanDocKind,
  PlanKind,
  PlanNodeDetailSchema,
  PlanResearchState,
  PlanStage,
} from "../protocol/gen/flotilla/board/v1/plan_pb";
import type { Question } from "../protocol/gen/flotilla/board/v1/questions_pb";
import { PUSH_REQUESTS_V1 } from "../protocol/ownerActions";
import { RESEARCH_GATE_V1 } from "../protocol/plan";
import { card } from "./boardFixtures";
import type { FakeDaemon } from "./fakeDaemon";
import { ownerAction } from "./ownerActionFixtures";
import { hudEpic, node, planClient, planNodes, planTree } from "./planFixtures";
import { licensingQuestion, onboardingQuestions } from "./questionFixtures";

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

const RESEARCH_DIR = "docs/initiatives/combat-onboarding/research";

/** PL-9: Combat onboarding, its two spikes done with documents, waiting for the owner. */
export function awaitingResearch(): PlanNode {
  return node({
    id: "PL-9",
    kind: PlanKind.INITIATIVE,
    parentId: "PL-1",
    title: "Combat onboarding",
    summary: "New players learn to block and dodge.",
    stage: PlanStage.RESEARCH,
    lead: "lead",
    boardId: "p1/onboarding",
    rank: "a2",
    version: 7n,
    docs: [
      { kind: PlanDocKind.RESEARCH, path: `${RESEARCH_DIR}/what-players-miss.md`, itemId: "CO-1" },
      { kind: PlanDocKind.RESEARCH, path: `${RESEARCH_DIR}/tutorial-study.md`, itemId: "CO-2" },
    ],
    research: { state: PlanResearchState.AWAITING, spikes: ["CO-1", "CO-2"] },
    gate: {
      stage: PlanStage.RESEARCH,
      submittedBy: "daemon",
      submittedAt: at("2025-03-04T14:02:00Z"),
      round: 1,
    },
  });
}

/** The plan with Combat onboarding's research waiting, a 0.24 service's. */
export function awaitingTree(): PlanTree {
  const nodes = planNodes().filter((n) => n.id !== "PL-9");
  const tree = planTree({ nodes: [...nodes, awaitingResearch()] });
  tree.researchGate = true;
  return tree;
}

/** One node's detail as `plan_node_get` answers it, with its cards and questions. */
function serveDetail(
  fake: FakeDaemon,
  target: PlanNode,
  cards: ReturnType<typeof card>[],
  questions: readonly Question[],
): FakeDaemon {
  return fake.onBoard("planNodeGet", (call) => {
    const id = call.case === "planNodeGet" ? call.value.id : "";
    return {
      case: "planNode",
      value: create(PlanNodeDetailSchema, {
        node: id === target.id ? target : undefined,
        cards: id === target.id ? cards : [],
        questions: id === target.id ? [...questions] : [],
      }),
    };
  });
}

/** A spike's research document, by its file. */
function researchDoc(path: string) {
  const title = path.endsWith("tutorial-study.md")
    ? "The 0.18 tutorial study"
    : "What players miss";
  const text = `# ${title}\n\nPlayers who block in the first minute keep blocking; those who don't, never start.\n\n## Findings\n\n- The dodge prompt shows after the first hit, too late.\n- Six of ten skip the tutorial text.\n`;
  const data = new TextEncoder().encode(text);
  return create(PlanDocContentSchema, {
    path,
    sha: "b71e004",
    data,
    total: BigInt(data.length),
    mime: "text/markdown",
  });
}

function spike(id: string, title: string) {
  return card({ id, title, type: ItemType.SPIKE, columnKey: "done", planNodeId: "PL-9" });
}

/** A 0.24 service with research waiting: `holding` adds an open blocking question. */
export function awaitingClient(holding = false): FakeDaemon {
  const fake = planClient(awaitingTree()).onBoard("planDocRead", (call) => ({
    case: "planDoc",
    value: researchDoc(call.case === "planDocRead" ? (call.value.path ?? "") : ""),
  }));
  fake.capabilities = [...fake.capabilities, RESEARCH_GATE_V1];
  return serveDetail(
    fake,
    awaitingResearch(),
    [spike("CO-1", "What players miss"), spike("CO-2", "The 0.18 tutorial study")],
    onboardingQuestions(holding),
  );
}

/** The HLD gate with a question answered on its spike CR-33 before it. */
export function hldClient(): FakeDaemon {
  const answered = { ...licensingQuestion(), itemId: "CR-33", planNodeId: "PL-8" };
  return serveDetail(
    planClient(),
    hudEpic(),
    [card({ id: "CR-33", title: "Off-screen threat cues", type: ItemType.SPIKE })],
    [answered],
  );
}

/** The HLD gate with the lead's push of its branch waiting for the owner. */
export function pushClient(): FakeDaemon {
  const fake = hldClient();
  fake.capabilities = [...fake.capabilities, PUSH_REQUESTS_V1];
  const push = ownerAction({
    id: "oa-7",
    kind: "push",
    proposed_by: "bot:lead",
    item_id: "CR-33",
    cwd: "/Users/~/Developer/game/.worktrees/hud-clarity",
    content: "git push origin plan/hud-clarity-hld:plan/hud-clarity-hld",
    reason: "The HLD's branch, so you can read it in its pull request",
    created_at: "2025-03-04T14:02:00Z",
    expires_at: "2025-03-05T14:02:00Z",
    push: {
      branch: "plan/hud-clarity-hld",
      target: "plan/hud-clarity-hld",
      repo: "me/game",
      commit: "4e1c9a2",
      files: 1,
      additions: 42,
      deletions: 3,
      item_id: "CR-33",
    },
  });
  return fake.onRequest("owner_action_list", () => ({
    type: "owner_actions",
    req_id: "1",
    actions: [push],
  }));
}
