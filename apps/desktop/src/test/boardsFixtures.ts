// A project with several boards (0.19.0) for stories and tests: Main, the
// release-gated board every project had before, and Recipes, a custom board
// from the Simple preset with a QA column the lead added. Drafts, a draft's
// attachments and changes, and the lead's proposals. Times are frozen near
// DRAWER_NOW so ages never drift.

import { create } from "@bufbuild/protobuf";
import type { MessageInitShape } from "@bufbuild/protobuf";
import { timestampFromDate } from "@bufbuild/protobuf/wkt";
import type { Board, BoardColumn, Item, ItemCard } from "../protocol/gen/hermes/board/v1/board_pb";
import {
  BoardColumnSchema,
  BoardSchema,
  BoardSettingsSchema,
  ColumnCategory,
  ColumnKind,
  DraftRevisionSchema,
  ItemAttachmentSchema,
  ItemSchema,
  ItemType,
  LabelGroupSchema,
  LinkKind,
  Priority,
  WipScope,
} from "../protocol/gen/hermes/board/v1/board_pb";
import type {
  BoardProposal,
  BoardSnapshot,
  DraftChanges,
  ItemDetail,
} from "../protocol/gen/hermes/board/v1/requests_pb";
import {
  BoardProposalSchema,
  BoardSnapshotSchema,
  DraftChangesSchema,
  ItemDetailSchema,
} from "../protocol/gen/hermes/board/v1/requests_pb";
import { card, defaultColumns } from "./boardFixtures";

const at = (iso: string) => timestampFromDate(new Date(iso));

/** A 96×60 screenshot: a title bar, a red error line, two grey lines. */
export const SCREENSHOT_PNG = Uint8Array.from(
  atob(
    "iVBORw0KGgoAAAANSUhEUgAAAGAAAAA8CAIAAAAWtijjAAAAiUlEQVR42u3WMQ3AIBRFUfxgoW6qAB0slVCBTReWjhggrDSfk1wFZ3h5KR+nJiUEgAABAgQIECBAGgI9b9MkQIAAAQIECNB+QF8pIQMECBAgQIAAAXIUAQECBAhQXKB63b8NECBA8YEECBAgI71+wgEBArQGSIAAAQIECBAgQIAECBAgQIAAhawD8rFxbgWfdmwAAAAASUVORK5CYII=",
  ),
  (c) => c.charCodeAt(0),
);

export const MAIN_ID = "p1/main";
export const RECIPES_ID = "b-recipes";

export function board(over: MessageInitShape<typeof BoardSchema> = {}): Board {
  return create(BoardSchema, {
    id: MAIN_ID,
    projectId: "p1",
    name: "Main",
    key: "H",
    ord: 0,
    isDefault: true,
    structurePolicy: "owner_confirms",
    flow: "release_gated",
    version: 4n,
    labelGroups: [
      create(LabelGroupSchema, { name: "platform", values: ["desktop", "ios", "daemon", "infra"] }),
    ],
    ...over,
  });
}

export function boards(): Board[] {
  return [
    board(),
    board({
      id: RECIPES_ID,
      name: "Recipes",
      key: "RB",
      ord: 1,
      isDefault: false,
      flow: "custom",
      description: "The recipe importer and the shopping list.",
      automations: { "pr.opened": "rb-qa", "pr.merged": "rb-done" },
      version: 9n,
    }),
    board({
      id: "b-old",
      name: "Spring cleanup",
      key: "SC",
      ord: 2,
      isDefault: false,
      archived: true,
      flow: "custom",
    }),
  ];
}

const KIND: Readonly<Partial<Record<ColumnCategory, ColumnKind>>> = {
  [ColumnCategory.INBOX]: ColumnKind.OPEN,
  [ColumnCategory.READY]: ColumnKind.OPEN,
  [ColumnCategory.DONE]: ColumnKind.DONE,
  [ColumnCategory.CANCELLED]: ColumnKind.CANCELLED,
};

/** Main's columns: the release-gated ones, with their board and kind. */
export function mainColumns(): BoardColumn[] {
  return defaultColumns().map((c) =>
    create(BoardColumnSchema, {
      ...c,
      boardId: MAIN_ID,
      kind: KIND[c.category] ?? ColumnKind.ACTIVE,
    }),
  );
}

/** Recipes' columns: Simple, plus QA with its rules. */
export function recipeColumns(): BoardColumn[] {
  type Over = {
    readonly wipLimit?: number;
    readonly wipScope?: WipScope;
    readonly visible?: boolean;
    readonly movers?: string[];
    readonly rules?: string[];
    readonly allowedFrom?: string[];
  };
  const spec: readonly [string, string, ColumnKind, Over][] = [
    ["rb-todo", "To do", ColumnKind.OPEN, {}],
    [
      "rb-doing",
      "Doing",
      ColumnKind.ACTIVE,
      {
        wipLimit: 3,
        wipScope: WipScope.COLUMN,
        movers: ["lead", "assignee"],
        rules: ["assignee_set"],
      },
    ],
    [
      "rb-qa",
      "QA",
      ColumnKind.ACTIVE,
      { movers: ["roles:tester"], rules: ["ac_all_checked", "link:pr"], allowedFrom: ["rb-doing"] },
    ],
    ["rb-done", "Done", ColumnKind.DONE, { movers: ["lead"] }],
    [
      "rb-cancelled",
      "Cancelled",
      ColumnKind.CANCELLED,
      { visible: false, movers: ["lead"], rules: ["reason"] },
    ],
  ];
  return spec.map(([key, name, kind, over], ord) =>
    create(BoardColumnSchema, {
      projectId: "p1",
      boardId: RECIPES_ID,
      key,
      name,
      ord: 10 + ord,
      kind,
      category: kind === ColumnKind.OPEN ? ColumnCategory.INBOX : ColumnCategory.DOING,
      visible: true,
      ...over,
    }),
  );
}

function recipeCards(): ItemCard[] {
  return [
    card({
      id: "RB-3",
      title: "Import from a URL",
      columnKey: "rb-todo",
      boardId: RECIPES_ID,
      rank: "a0",
    }),
    card({
      id: "RB-4",
      title: "Scale servings",
      columnKey: "rb-todo",
      boardId: RECIPES_ID,
      rank: "a1",
      type: ItemType.CHORE,
    }),
    card({
      id: "RB-7",
      title: "Shopping list by aisle",
      columnKey: "rb-doing",
      boardId: RECIPES_ID,
      assignee: "b1",
      acChecked: 1,
      acTotal: 3,
    }),
    card({
      id: "RB-9",
      title: "Fix sort order",
      columnKey: "rb-qa",
      boardId: RECIPES_ID,
      type: ItemType.BUG,
      assignee: "b2",
      acChecked: 2,
      acTotal: 2,
    }),
    card({ id: "RB-1", title: "Recipe cards", columnKey: "rb-done", boardId: RECIPES_ID }),
  ];
}

function drafts(): ItemCard[] {
  return [
    card({
      id: "RB-20",
      title: "Unit conversion (cups ↔ grams)",
      columnKey: "rb-todo",
      boardId: RECIPES_ID,
      draft: true,
      version: 6n,
    }),
    card({
      id: "RB-21",
      title: "Print a recipe",
      columnKey: "rb-todo",
      boardId: RECIPES_ID,
      draft: true,
      type: ItemType.CHORE,
    }),
  ];
}

/** The lead's applied change and an open proposal. */
export function proposals(): BoardProposal[] {
  return [
    create(BoardProposalSchema, {
      id: "bp-2",
      boardId: RECIPES_ID,
      summary: "move items to rb-done on pr.merged",
      proposedBy: "bot:lead",
      decisionId: "dec-2",
      state: "open",
      at: at("2026-10-05T16:40:00Z"),
      change: {
        case: "automationSet",
        value: { boardId: RECIPES_ID, version: 9n, fact: "pr.merged", columnKey: "rb-done" },
      },
    }),
    create(BoardProposalSchema, {
      id: "bp-1",
      boardId: RECIPES_ID,
      summary: "add column “QA” with rules ac_all_checked",
      proposedBy: "bot:lead",
      state: "applied",
      at: at("2026-10-05T14:00:00Z"),
      change: {
        case: "columnAdd",
        value: {
          boardId: RECIPES_ID,
          version: 7n,
          name: "QA",
          kind: ColumnKind.ACTIVE,
          after: "rb-doing",
          rules: ["ac_all_checked"],
        },
      },
    }),
  ];
}

export function boardsSnapshot(
  over: MessageInitShape<typeof BoardSnapshotSchema> = {},
): BoardSnapshot {
  return create(BoardSnapshotSchema, {
    settings: create(BoardSettingsSchema, { projectId: "p1", key: "H", staleAfterHours: 24 }),
    boards: boards(),
    columns: [...mainColumns(), ...recipeColumns()],
    cards: recipeCards(),
    drafts: drafts(),
    proposals: proposals(),
    seq: 3n,
    canRule: true,
    ...over,
  });
}

/** RB-20, a draft on Recipes. */
export function draftItem(over: MessageInitShape<typeof ItemSchema> = {}): Item {
  return create(ItemSchema, {
    id: "RB-20",
    seq: 20,
    type: ItemType.FEATURE,
    title: "Unit conversion (cups ↔ grams)",
    description:
      "## User/problem\n\nThe import breaks on this site:\n\n![the error](attachment:att-shot)\n\n![first try](attachment:att-gone)",
    priority: Priority.P2,
    columnKey: "rb-todo",
    boardId: RECIPES_ID,
    state: "draft",
    acceptanceCriteria: [
      { idx: 0, text: "Cups convert to grams", checked: false },
      { idx: 1, text: "Grams convert to cups", checked: false },
    ],
    createdBy: "user",
    version: 6n,
    ...over,
  });
}

/** RB-20 as a draft: a screenshot inline, a spec, a link and one detached. */
export function draftDetail(): ItemDetail {
  return create(ItemDetailSchema, {
    item: draftItem(),
    links: [
      {
        itemId: "RB-20",
        kind: LinkKind.THREAD,
        ref: "lead",
        label: "Team Lead",
        createdBy: "user",
      },
    ],
    attachments: [
      create(ItemAttachmentSchema, {
        id: "att-shot",
        itemId: "RB-20",
        kind: "image",
        name: "import-error.png",
        mime: "image/png",
        bytes: 194n,
        sha256: "ab12",
        path: "items/RB-20/ab12-import-error.png",
        caption: "The error",
        addedBy: "user",
        at: at("2026-10-05T15:00:00Z"),
      }),
      create(ItemAttachmentSchema, {
        id: "att-chart",
        itemId: "RB-20",
        kind: "file",
        name: "chart.pdf",
        mime: "application/pdf",
        bytes: 1_310_720n,
        sha256: "cd34",
        path: "items/RB-20/cd34-chart.pdf",
        addedBy: "user",
        at: at("2026-10-05T15:01:00Z"),
      }),
      create(ItemAttachmentSchema, {
        id: "att-spec",
        itemId: "RB-20",
        kind: "doc",
        name: "RB-20-spec.md",
        path: "specs/RB-20-spec.md",
        caption: "Spec",
        addedBy: "bot:lead",
        at: at("2026-10-05T16:30:00Z"),
      }),
      create(ItemAttachmentSchema, {
        id: "att-gone",
        itemId: "RB-20",
        kind: "image",
        name: "first-try.png",
        mime: "image/png",
        bytes: 194n,
        addedBy: "user",
        removed: true,
        at: at("2026-10-05T14:00:00Z"),
      }),
    ],
    invited: ["lead"],
  });
}

/** The lead's three changes since the owner looked (seen up to 2). */
export function draftChanges(): DraftChanges {
  const rev = (id: bigint, field: string, from: string | undefined, to: string | undefined) =>
    create(DraftRevisionSchema, {
      id,
      itemId: "RB-20",
      actor: "bot:lead",
      field,
      from,
      to,
      at: at("2026-10-05T16:30:00Z"),
    });
  return create(DraftChangesSchema, {
    itemId: "RB-20",
    seen: 2n,
    revisions: [
      create(DraftRevisionSchema, {
        id: 1n,
        itemId: "RB-20",
        actor: "user",
        field: "description",
        from: "",
        to: "x",
      }),
      rev(2n, "title", "Unit conversion", "Convert cups and grams"),
      rev(3n, "title", "Convert cups and grams", "Unit conversion (cups ↔ grams)"),
      rev(4n, "acceptance_criteria", "", "Cups convert to grams\nGrams convert to cups"),
      rev(5n, "attachment", undefined, "RB-20-spec.md"),
    ],
  });
}
