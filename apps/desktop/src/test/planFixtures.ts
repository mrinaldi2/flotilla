// The plan of a game project (0.21.0, #43 and its canvas): improving combat
// UX through its initiatives, epics and features, with an HLD waiting for
// the owner. Shapes follow crates/bus/fixtures/board/messages/*plan*.

import { create } from "@bufbuild/protobuf";
import type { MessageInitShape } from "@bufbuild/protobuf";
import { timestampFromDate } from "@bufbuild/protobuf/wkt";
import {
  BoardColumnSchema,
  ColumnCategory,
  ColumnKind,
  ItemType,
} from "../protocol/gen/flotilla/board/v1/board_pb";
import type { BoardColumn, ItemCard } from "../protocol/gen/flotilla/board/v1/board_pb";
import {
  PlanDocContentSchema,
  PlanDocKind,
  PlanKind,
  PlanNodeDetailSchema,
  PlanNodeSchema,
  PlanResearchState,
  PlanResultSchema,
  PlanStage,
  PlanTreeSchema,
} from "../protocol/gen/flotilla/board/v1/plan_pb";
import type {
  PlanDocContent,
  PlanNode,
  PlanNodeDetail,
  PlanResult,
  PlanTree,
} from "../protocol/gen/flotilla/board/v1/plan_pb";
import { BoardUnwatchedSchema } from "../protocol/gen/flotilla/board/v1/requests_pb";
import { ITEM_TYPES_V2, PLAN_V1, COLUMN_RESEARCH } from "../protocol/plan";
import { card } from "./boardFixtures";
import { FakeDaemon } from "./fakeDaemon";

const at = (iso: string) => timestampFromDate(new Date(iso));

export const PLAN_PROJECT = "p1";
export const PLAN_BOARD = "p1/combat-readability";

function node(over: MessageInitShape<typeof PlanNodeSchema>): PlanNode {
  return create(PlanNodeSchema, {
    projectId: PLAN_PROJECT,
    createdBy: "user",
    createdAt: at("2026-10-10T08:00:00Z"),
    updatedAt: at("2026-10-12T08:00:00Z"),
    version: 3n,
    rank: "a0",
    ...over,
  });
}

const EPIC_DIR = "docs/initiatives/combat-readability/epics";

/** The HLD the lead submitted for Combat HUD clarity, waiting for the owner. */
export function hudEpic(): PlanNode {
  return node({
    id: "PL-8",
    kind: PlanKind.EPIC,
    parentId: "PL-2",
    title: "Combat HUD clarity",
    summary: "Players read health, stamina and threats at a glance.",
    stage: PlanStage.HLD,
    lead: "lead",
    boardId: PLAN_BOARD,
    rank: "a2",
    version: 6n,
    docs: [
      {
        kind: PlanDocKind.DESIGN,
        path: `${EPIC_DIR}/hud-clarity/design/design.md`,
        approvedSha: "9b2f0c1",
        approvedAt: at("2026-10-12T09:00:00Z"),
      },
      { kind: PlanDocKind.HLD, path: `${EPIC_DIR}/hud-clarity/hld.md` },
    ],
    research: {
      state: PlanResearchState.CLOSED,
      spikes: ["CR-33", "CR-34"],
      closedAt: at("2026-10-11T16:00:00Z"),
    },
    gate: {
      stage: PlanStage.HLD,
      docPath: `${EPIC_DIR}/hud-clarity/hld.md`,
      sha: "4e1c9a2",
      prNumber: 18,
      submittedBy: "bot:lead",
      submittedAt: at("2026-10-17T10:00:00Z"),
      note: "Second round: risks added.",
      decisionId: "d-41",
      round: 2,
      children: [
        { title: "Readable health and stamina bars", summary: "Presenter and bar widgets" },
        { title: "Off-screen enemy indicators", summary: "Edge projection service" },
        { title: "Target lock marker", summary: "Follows the locked enemy" },
      ],
    },
  });
}

/** The epic in Design: research closed with three spikes. */
export function telegraphEpic(): PlanNode {
  return node({
    id: "PL-4",
    kind: PlanKind.EPIC,
    parentId: "PL-2",
    title: "Enemy attack telegraphing",
    summary: "Players see every attack coming.",
    stage: PlanStage.DESIGN,
    lead: "b1",
    boardId: PLAN_BOARD,
    version: 5n,
    docs: [
      {
        kind: PlanDocKind.RESEARCH,
        path: `${EPIC_DIR}/attack-telegraphing/research/references.md`,
        itemId: "CR-12",
      },
      {
        kind: PlanDocKind.RESEARCH,
        path: `${EPIC_DIR}/attack-telegraphing/research/prototype-results.md`,
        itemId: "CR-14",
      },
      { kind: PlanDocKind.DESIGN, path: `${EPIC_DIR}/attack-telegraphing/design/design.md` },
    ],
    research: {
      state: PlanResearchState.CLOSED,
      spikes: ["CR-12", "CR-14", "CR-15"],
      closedAt: at("2026-10-14T16:00:00Z"),
    },
  });
}

export function planNodes(): PlanNode[] {
  return [
    node({
      id: "PL-1",
      kind: PlanKind.GOAL,
      title: "Improve combat UX",
      summary: "Combat satisfaction from 3.1 to 4+/5 in playtests.",
      stage: PlanStage.ACTIVE,
      due: "Playtest round 2",
    }),
    node({
      id: "PL-2",
      kind: PlanKind.INITIATIVE,
      parentId: "PL-1",
      title: "Combat readability",
      summary: "Players lose track of what hits them.",
      stage: PlanStage.RUNNING,
      boardId: PLAN_BOARD,
      cardsTotal: 24,
      docs: [
        {
          kind: PlanDocKind.PRD,
          path: "docs/initiatives/combat-readability/prd.md",
          approvedSha: "77ad310",
          approvedAt: at("2026-10-11T08:00:00Z"),
          changedSinceApproval: true,
        },
      ],
      research: { state: PlanResearchState.SKIPPED, skippedReason: "Playtest notes cover it." },
    }),
    telegraphEpic(),
    node({
      id: "PL-5",
      kind: PlanKind.EPIC,
      parentId: "PL-2",
      title: "Hit feedback",
      stage: PlanStage.RUNNING,
      boardId: PLAN_BOARD,
      rank: "a1",
    }),
    node({
      id: "PL-6",
      kind: PlanKind.FEATURE,
      parentId: "PL-5",
      title: "Hitstop on heavy hits",
      stage: PlanStage.BUILD,
      boardId: PLAN_BOARD,
      cardsTotal: 5,
      cardsDone: 3,
    }),
    node({
      id: "PL-7",
      kind: PlanKind.FEATURE,
      parentId: "PL-5",
      title: "Damage numbers",
      stage: PlanStage.LLD,
      boardId: PLAN_BOARD,
      rank: "a1",
    }),
    hudEpic(),
    node({
      id: "PL-3",
      kind: PlanKind.INITIATIVE,
      parentId: "PL-1",
      title: "Controls and responsiveness",
      stage: PlanStage.PRD,
      boardId: "p1/controls",
      rank: "a1",
    }),
    node({
      id: "PL-9",
      kind: PlanKind.INITIATIVE,
      parentId: "PL-1",
      title: "Combat onboarding",
      stage: PlanStage.RESEARCH,
      boardId: "p1/onboarding",
      rank: "a2",
      research: { state: PlanResearchState.OPEN, spikes: ["CO-1"] },
    }),
  ];
}

export function planTree(over: Partial<{ enabled: boolean }> = {}): PlanTree {
  return create(PlanTreeSchema, {
    projectId: PLAN_PROJECT,
    enabled: over.enabled ?? true,
    hasRepo: true,
    nodes: planNodes(),
    guidelines: [{ kind: PlanDocKind.GUIDELINE, path: "docs/guidelines/game-feel.md" }],
  });
}

function spike(id: string, title: string): ItemCard {
  return card({ id, title, type: ItemType.SPIKE, columnKey: "done", planNodeId: "PL-4" });
}

function planNodeDetail(id: string): PlanNodeDetail {
  const found = planNodes().find((n) => n.id === id);
  return create(PlanNodeDetailSchema, {
    node: found,
    history: [
      { at: at("2026-10-11T08:00:00Z"), actor: "user", kind: "approved", stage: PlanStage.PRD },
      {
        at: at("2026-10-14T16:00:00Z"),
        actor: "daemon",
        kind: "research_closed",
        stage: PlanStage.RESEARCH,
      },
      {
        at: at("2026-10-16T12:00:00Z"),
        actor: "user",
        kind: "sent_back",
        stage: PlanStage.HLD,
        note: "Add the risks.",
      },
      {
        at: at("2026-10-17T10:00:00Z"),
        actor: "bot:lead",
        kind: "submitted",
        stage: PlanStage.HLD,
      },
    ],
    cards:
      id !== "PL-4"
        ? []
        : [
            spike("CR-12", "How other games telegraph attacks"),
            spike("CR-14", "Prototype three telegraph styles"),
            spike("CR-15", "Anticipation cues in the literature"),
            card({
              id: "CR-30",
              title: "Telegraph language",
              type: ItemType.DESIGN,
              columnKey: "doing",
              planNodeId: id,
            }),
          ],
  });
}

const HLD = `# Combat HUD clarity · HLD

## Context

The approved design asks for a HUD that reads in under a second at the edge of vision.

## Components

- A **HUD presenter** listens to player and enemy events and owns every widget.
- An **off-screen indicator** service projects threats onto the screen edge.
- A **target marker** follows the locked enemy.

## Risks

Clutter on small screens: every widget can be hidden in Accessibility settings.
`;

const HLD_DIFF = `@@ -9,3 +9,7 @@
 - A **target marker** follows the locked enemy.
+
+## Risks
+
+Clutter on small screens: every widget can be hidden in Accessibility settings.
`;

function planDoc(path: string, diff?: string): PlanDocContent {
  const data = new TextEncoder().encode(HLD);
  return create(PlanDocContentSchema, {
    path,
    sha: "4e1c9a2",
    data,
    total: BigInt(data.length),
    mime: "text/markdown",
    diff,
  });
}

export function planned(next: PlanNode, created: readonly string[] = []): PlanResult {
  return create(PlanResultSchema, {
    outcome: { case: "done", value: next },
    created: [...created],
  });
}

/** The initiative board's columns, Research between Ready and Doing. */
export function initiativeColumns(): BoardColumn[] {
  const spec: [string, string, ColumnCategory, ColumnKind][] = [
    ["inbox", "Inbox", ColumnCategory.INBOX, ColumnKind.OPEN],
    ["ready", "Ready", ColumnCategory.READY, ColumnKind.OPEN],
    ["research", "Research", ColumnCategory.RESEARCH, ColumnKind.ACTIVE],
    ["doing", "Doing", ColumnCategory.DOING, ColumnKind.ACTIVE],
    ["review", "Review", ColumnCategory.REVIEW, ColumnKind.ACTIVE],
    ["done", "Done", ColumnCategory.DONE, ColumnKind.DONE],
  ];
  return spec.map(([key, name, category, kind], ord) =>
    create(BoardColumnSchema, {
      projectId: PLAN_PROJECT,
      boardId: PLAN_BOARD,
      key,
      name,
      ord,
      category,
      kind,
      visible: true,
    }),
  );
}

/** A Flotilla service that serves the plan: the tree, a node, its documents and rulings. */
export function planClient(tree: PlanTree = planTree()): FakeDaemon {
  const fake = new FakeDaemon()
    .onBoard("planGet", () => ({ case: "plan", value: tree }))
    .onBoard("planNodeGet", (call) => ({
      case: "planNode",
      value: planNodeDetail(call.case === "planNodeGet" ? (call.value.id ?? "") : ""),
    }))
    .onBoard("planDocRead", (call) => ({
      case: "planDoc",
      value: planDoc(
        call.case === "planDocRead" ? (call.value.path ?? "") : "",
        call.case === "planDocRead" && call.value.diffFrom !== undefined ? HLD_DIFF : undefined,
      ),
    }))
    .onBoard("boardWatch", () => {
      throw new Error("no board in this story");
    })
    .onBoard("boardUnwatch", () => ({
      case: "unwatched",
      value: create(BoardUnwatchedSchema, { projectId: PLAN_PROJECT }),
    }));
  fake.grants = ["read", "control", "approve"];
  fake.capabilities = [...fake.capabilities, PLAN_V1, ITEM_TYPES_V2, COLUMN_RESEARCH];
  return fake;
}
