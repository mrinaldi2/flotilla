// An initiative board's cards in the drawer (0.21.0, canvas D3 and D4): a
// task held in Research with the design it builds, and that done Design card
// with its documents, which the owner attaches to tasks.

import { create } from "@bufbuild/protobuf";
import { timestampFromDate } from "@bufbuild/protobuf/wkt";
import {
  BoardSettingsSchema,
  ItemAttachmentSchema,
  ItemSchema,
  ItemType,
  LinkKind,
  Priority,
} from "../protocol/gen/flotilla/board/v1/board_pb";
import {
  BoardSnapshotSchema,
  ItemDetailSchema,
  MoveCheckSchema,
} from "../protocol/gen/flotilla/board/v1/requests_pb";
import type {
  BoardSnapshot,
  ItemDetail,
  MoveCheck,
} from "../protocol/gen/flotilla/board/v1/requests_pb";
import { card, unmet } from "./boardFixtures";
import { board } from "./boardsFixtures";
import { FakeDaemon } from "./fakeDaemon";
import { initiativeColumns, PLAN_BOARD } from "./planFixtures";
import { COLUMN_RESEARCH, ITEM_TYPES_V2, PLAN_V1 } from "../protocol/plan";

function doc(itemId: string, id: string, name: string, mime: string) {
  return create(ItemAttachmentSchema, {
    id,
    itemId,
    kind: "doc",
    name,
    mime,
    path: `design/${name}`,
  });
}

/** CR-28: a task in Research whose code research isn't written yet, building CR-19. */
function researchTask(): ItemDetail {
  return create(ItemDetailSchema, {
    item: create(ItemSchema, {
      id: "CR-28",
      seq: 28,
      type: ItemType.TASK,
      title: "Play-mode test for hitstop duration",
      description: "## Code research\n\n",
      priority: Priority.P2,
      columnKey: "research",
      boardId: PLAN_BOARD,
      assignee: "b2",
      planNodeId: "PL-6",
      stateEnteredAt: timestampFromDate(new Date("2026-10-05T09:00:00Z")),
      acceptanceCriteria: [
        { idx: 0, text: "Hitstop lasts 60–120 ms, scaled by damage", checked: false },
        { idx: 1, text: "It can be turned off in settings", checked: false },
      ],
      version: 4n,
    }),
    links: [
      {
        itemId: "CR-28",
        kind: LinkKind.DESIGN,
        ref: "CR-19",
        label: "Hitstop feel and timings",
        createdBy: "user",
      },
    ],
  });
}

/** CR-19: the done Design card, its spec and mockups attached. */
export function designCard(): ItemDetail {
  return create(ItemDetailSchema, {
    item: create(ItemSchema, {
      id: "CR-19",
      seq: 19,
      type: ItemType.DESIGN,
      title: "Hitstop feel and timings",
      description: "Timings per attack tier, and how the screen reads during hitstop.",
      priority: Priority.P2,
      columnKey: "done",
      boardId: PLAN_BOARD,
      assignee: "b1",
      stateEnteredAt: timestampFromDate(new Date("2026-10-04T12:00:00Z")),
      planNodeId: "PL-6",
      version: 9n,
    }),
    attachments: [
      doc("CR-19", "a-1", "design.md", "text/markdown"),
      doc("CR-19", "a-2", "timing-curve.html", "text/html"),
      doc("CR-19", "a-3", "mockup-1.png", "image/png"),
      doc("CR-19", "a-4", "mockup-2.png", "image/png"),
    ],
  });
}

/** Doing refuses CR-28 until its code research is written. */
function researchCheck(): MoveCheck {
  return create(MoveCheckSchema, {
    itemId: "CR-28",
    columns: [
      {
        columnKey: "doing",
        unmet: [
          unmet(
            "start.research",
            "Write what you found in the code under Code research, or attach a research document.",
          ),
        ],
      },
    ],
  });
}

/** The board's open tasks, which a done design attaches to. */
export function openTasks() {
  return [
    card({
      id: "CR-27",
      title: "Add hitstop fields to the feedback profile",
      columnKey: "research",
      boardId: PLAN_BOARD,
    }),
    card({
      id: "CR-28",
      title: "Play-mode test for hitstop duration",
      columnKey: "research",
      boardId: PLAN_BOARD,
    }),
  ];
}

/** A 0.21 service that answers the two cards and their move checks. */
export function researchClient(): FakeDaemon {
  const fake = new FakeDaemon()
    .onBoard("itemGet", (call) => ({
      case: "item",
      value: call.case === "itemGet" && call.value.id === "CR-19" ? designCard() : researchTask(),
    }))
    .onBoard("itemMoveCheck", () => ({ case: "moveCheck", value: researchCheck() }))
    .onBoard("boardWatch", () => ({ case: "board", value: initiativeSnapshot() }))
    .onBoard("boardGet", () => ({ case: "board", value: initiativeSnapshot() }));
  fake.grants = ["read", "control", "approve"];
  fake.capabilities = [...fake.capabilities, PLAN_V1, ITEM_TYPES_V2, COLUMN_RESEARCH];
  return fake;
}

const ON_BOARD = { boardId: PLAN_BOARD };

/** The initiative's board: its cards across Inbox to Done (canvas D3). */
function initiativeSnapshot(): BoardSnapshot {
  return create(BoardSnapshotSchema, {
    settings: create(BoardSettingsSchema, { projectId: "p1", key: "CR", staleAfterHours: 24 }),
    boards: [
      board({
        id: PLAN_BOARD,
        name: "Combat readability",
        key: "CR",
        isDefault: false,
        flow: "custom",
        labelGroups: [],
      }),
    ],
    columns: initiativeColumns(),
    cards: [
      card({
        ...ON_BOARD,
        id: "CR-31",
        type: ItemType.BUG,
        title: "Hitstop also freezes UI animations",
        columnKey: "inbox",
        planNodeId: "PL-6",
      }),
      card({
        ...ON_BOARD,
        id: "CR-33",
        type: ItemType.SPIKE,
        title: "Off-screen threat cues: three options",
        columnKey: "ready",
        planNodeId: "PL-8",
      }),
      card({
        ...ON_BOARD,
        id: "CR-27",
        title: "Add hitstop fields to the feedback profile",
        columnKey: "research",
        assignee: "b2",
        planNodeId: "PL-6",
      }),
      card({
        ...ON_BOARD,
        id: "CR-28",
        title: "Play-mode test for hitstop duration",
        columnKey: "research",
        planNodeId: "PL-6",
        rank: "a1",
      }),
      card({
        ...ON_BOARD,
        id: "CR-24",
        title: "Add the hitstop service",
        columnKey: "doing",
        assignee: "b2",
        planNodeId: "PL-6",
      }),
      card({
        ...ON_BOARD,
        id: "CR-30",
        type: ItemType.DESIGN,
        title: "Telegraph language",
        columnKey: "doing",
        assignee: "b1",
        planNodeId: "PL-4",
        rank: "a1",
      }),
      card({
        ...ON_BOARD,
        id: "CR-25",
        title: "Telemetry event hit_feedback_shown",
        columnKey: "review",
        planNodeId: "PL-6",
      }),
      card({
        ...ON_BOARD,
        id: "CR-19",
        type: ItemType.DESIGN,
        title: "Hitstop feel and timings",
        columnKey: "done",
        planNodeId: "PL-6",
      }),
      card({
        ...ON_BOARD,
        id: "CR-14",
        type: ItemType.SPIKE,
        title: "Prototype three telegraph styles",
        columnKey: "done",
        planNodeId: "PL-4",
        rank: "a1",
      }),
    ],
    seq: 4n,
    canRule: true,
  });
}
