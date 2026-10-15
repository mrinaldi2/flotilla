// The pure part of a column row in Board settings (0.19.0): where a move
// puts it, what a typed WIP limit changes, and its rules in one line.

import type { BoardColumn, ColumnKind } from "../../../protocol/gen/flotilla/board/v1/board_pb";
import type { Policy } from "./ColumnRules";
import { moverWords, ruleWords } from "./rules";

/** A change to one column, as `column_update` takes it. */
export interface ColumnChange {
  readonly name?: string;
  readonly kind?: ColumnKind;
  readonly wipLimit?: number;
  readonly clearWipLimit?: boolean;
  readonly visible?: boolean;
  /** "" moves it first. */
  readonly after?: string;
  readonly policy?: Policy;
}

/** The `after` that moves `column` one place up or down, or null at the end. */
export function afterFor(
  columns: readonly BoardColumn[],
  column: BoardColumn,
  by: -1 | 1,
): string | null {
  const at = columns.findIndex((c) => c.key === column.key);
  const to = at + by;
  if (at < 0 || to < 0 || to >= columns.length) {
    return null;
  }
  // Up: after the column two above (or first); down: after the one below.
  return by === -1 ? (columns[to - 1]?.key ?? "") : (columns[to]?.key ?? null);
}

/** What a typed WIP limit changes: clears it, sets it, or nothing. */
export function limitChange(text: string, current: number | undefined): ColumnChange | null {
  if (text.trim() === "") {
    return current === undefined ? null : { clearWipLimit: true };
  }
  const limit = Number(text);
  return Number.isInteger(limit) && limit > 0 && limit !== current ? { wipLimit: limit } : null;
}

/** "The lead · Has an assignee · from Doing". */
export function policyLine(
  column: BoardColumn,
  columns: readonly BoardColumn[],
  custom: boolean,
): string {
  if (!custom) {
    return "Built-in release-gated rules";
  }
  const from = column.allowedFrom.map((k) => columns.find((c) => c.key === k)?.name ?? k);
  return [
    moverWords(column.movers),
    ...column.rules.map(ruleWords),
    from.length === 0 ? "" : `from ${from.join(", ")}`,
  ]
    .filter(Boolean)
    .join(" · ");
}
