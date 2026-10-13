// The item drawer's words (H-018 §3.6): the state stepper, the next step
// from the guard check, and one line per history event. Every state is a
// word, never colour alone.

import { timestampDate } from "@bufbuild/protobuf/wkt";
import type { Timestamp } from "@bufbuild/protobuf/wkt";
import type { BoardColumn, ItemEvent, Unmet } from "../../../protocol/gen/hermes/board/v1/board_pb";
import {
  ColumnCategory,
  ColumnKind,
  ItemEventKind,
} from "../../../protocol/gen/hermes/board/v1/board_pb";
import type { MoveCheck } from "../../../protocol/gen/hermes/board/v1/requests_pb";

/** The flow's steps, in order; Cancelled is off the path. */
export const STEPS: readonly { readonly category: ColumnCategory; readonly word: string }[] = [
  { category: ColumnCategory.INBOX, word: "Inbox" },
  { category: ColumnCategory.READY, word: "Ready" },
  { category: ColumnCategory.DOING, word: "Doing" },
  { category: ColumnCategory.REVIEW, word: "Review" },
  { category: ColumnCategory.VERIFY, word: "Verify" },
  { category: ColumnCategory.APPROVAL, word: "Awaiting owner" },
  { category: ColumnCategory.DEPLOYING, word: "Deploying" },
  { category: ColumnCategory.DONE, word: "Done" },
];

export function when(at: Timestamp | undefined): string {
  if (at === undefined) {
    return "";
  }
  return timestampDate(at).toLocaleString([], {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

/** "31h", "3d": how long since `at`, for "Doing for 31h". */
export function age(at: Timestamp | undefined, now: number): string {
  if (at === undefined) {
    return "";
  }
  const hours = Math.max(0, Math.floor((now - timestampDate(at).getTime()) / 3_600_000));
  return hours < 48 ? `${hours}h` : `${Math.floor(hours / 24)}d`;
}

export interface NextStep {
  /** The column the next forward move goes to, when there is one. */
  readonly to: BoardColumn | undefined;
  /** What stands in the way, for this connection. */
  readonly unmet: readonly Unmet[];
}

interface Flow {
  /** The steps in order, each a word. */
  readonly steps: readonly { readonly key: string; readonly word: string }[];
  /** Where the item is among them; -1 off the path. */
  readonly reached: number;
  /** The column the next forward move goes to. */
  readonly next: BoardColumn | undefined;
}

/**
 * The item's path (0.19.0). A release-gated board (or one from a home
 * before 0.19) has the canonical steps; a custom board's path is its own
 * visible columns in order, Cancelled aside. Only release-gated columns
 * have a Ready category.
 */
export function flowOf(columns: readonly BoardColumn[], current: BoardColumn | undefined): Flow {
  const board = columns.filter((c) => current !== undefined && c.boardId === current.boardId);
  const custom =
    current !== undefined &&
    current.boardId !== "" &&
    !board.some((c) => c.category === ColumnCategory.READY);
  if (custom) {
    const path = board.filter((c) => c.visible && c.kind !== ColumnKind.CANCELLED);
    // oxlint-disable-next-line unicorn/no-array-sort
    path.sort((a, b) => a.ord - b.ord);
    const reached = path.findIndex((c) => c.key === current.key);
    return {
      steps: path.map((c) => ({ key: c.key, word: c.name })),
      reached,
      next: reached < 0 ? undefined : path[reached + 1],
    };
  }
  const reached = STEPS.findIndex((s) => s.category === current?.category);
  const following = reached < 0 ? undefined : STEPS[reached + 1];
  return {
    steps: STEPS.map((s) => ({ key: s.word, word: s.word })),
    reached,
    next: following ? columns.find((c) => c.category === following.category) : undefined,
  };
}

/** The next forward move and its unmet guards (`item_move_check`). */
export function nextStep(
  columns: readonly BoardColumn[],
  current: BoardColumn | undefined,
  check: MoveCheck | null,
): NextStep {
  const to = flowOf(columns, current).next;
  const unmet = to ? (check?.columns.find((c) => c.columnKey === to.key)?.unmet ?? []) : [];
  return { to, unmet };
}

interface Words {
  readonly actor: string;
  readonly who: (actor: string) => string;
  readonly columnName: (key: string) => string;
}

/** " (note)", ": note", or nothing. */
function noted(note: string | undefined, lead: string): string {
  return note ? `${lead}${note}` : "";
}

/** Each kind of history event in words; anything else falls back below. */
const EVENT_WORDS: Partial<Record<ItemEventKind, (e: ItemEvent, w: Words) => string>> = {
  [ItemEventKind.CREATED]: (e, w) => `${w.actor} created it${noted(e.note, " · ")}`,
  [ItemEventKind.MOVED]: (e, w) =>
    `${w.actor} moved ${w.columnName(e.from ?? "")} → ${w.columnName(e.to ?? "")}${noted(e.note, ": ")}`,
  [ItemEventKind.EDITED]: (e, w) => `${w.actor} changed ${e.field ?? "it"}${noted(e.to, " to ")}`,
  [ItemEventKind.LINKED]: (e, w) => `${w.actor} linked ${e.field ?? "a"}${noted(e.to, " ")}`,
  [ItemEventKind.ASSIGNED]: (e, w) =>
    e.to ? `${w.actor} assigned ${w.who(`bot:${e.to}`)}` : `${w.actor} unassigned it`,
  [ItemEventKind.BLOCKED]: (e, w) =>
    e.note ? `${w.actor} blocked it: ${e.note}` : `${w.actor} unblocked it`,
  [ItemEventKind.RANKED]: (_e, w) => `${w.actor} reordered it`,
  [ItemEventKind.COMMENTED]: (_e, w) => `${w.actor} commented`,
};

/** "Team Lead moved Ready → Doing": one history event in words. */
export function eventLine(
  event: ItemEvent,
  who: (actor: string) => string,
  columnName: (key: string) => string,
): string {
  const words = { actor: who(event.actor), who, columnName };
  const line = EVENT_WORDS[event.kind];
  return line ? line(event, words) : `${words.actor}: ${event.note ?? "changed it"}`;
}
