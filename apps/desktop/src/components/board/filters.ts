// The board's filter bar: bot, type and the board's label groups.
// Filtered-out cards are hidden, but column counts still read "2 of 5".

import type { ItemCard } from "../../protocol/gen/flotilla/board/v1/board_pb";
import { ItemType } from "../../protocol/gen/flotilla/board/v1/board_pb";
import { labelGroupOf } from "./labelGroups";

export interface BoardFilters {
  /** Assignee bot ids. */
  readonly bots: readonly string[];
  /** Whole labels (`platform:ios`). */
  readonly labels: readonly string[];
  readonly types: readonly ItemType[];
}

export const NO_FILTERS: BoardFilters = { bots: [], labels: [], types: [] };

export function isFiltering(filters: BoardFilters): boolean {
  return filters.bots.length + filters.labels.length + filters.types.length > 0;
}

/** Each label group filtered on needs one of its labels picked on the card. */
function matchesLabels(labels: readonly string[], wanted: readonly string[]): boolean {
  const groups = new Set(wanted.map(labelGroupOf));
  return [...groups].every((group) =>
    wanted.some((label) => labelGroupOf(label) === group && labels.includes(label)),
  );
}

/** Every set filter must match; labels match on any overlap within a group. */
export function matchesFilters(card: ItemCard, filters: BoardFilters): boolean {
  if (
    filters.bots.length > 0 &&
    (card.assignee === undefined || !filters.bots.includes(card.assignee))
  ) {
    return false;
  }
  if (!matchesLabels(card.labels, filters.labels)) {
    return false;
  }
  return filters.types.length === 0 || filters.types.includes(card.type);
}

/** Adds `value` to `list`, or removes it when present. */
export function toggled<T>(list: readonly T[], value: T): readonly T[] {
  return list.includes(value) ? list.filter((item) => item !== value) : [...list, value];
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function stringsIn(value: unknown): readonly string[] {
  return Array.isArray(value) ? value.filter((s): s is string => typeof s === "string") : [];
}

function numbersIn<T extends number>(value: unknown, known: (n: number) => n is T): readonly T[] {
  return Array.isArray(value) ? value.filter((n): n is T => typeof n === "number" && known(n)) : [];
}

const isItemType = (n: number): n is ItemType => n in ItemType && n !== ItemType.UNSPECIFIED;

/** Narrows stored JSON back to filters, dropping anything unknown (such as
 * the platform numbers saved before 0.19). */
export function parseFilters(value: unknown): BoardFilters {
  if (!isRecord(value)) {
    return NO_FILTERS;
  }
  return {
    bots: stringsIn(value["bots"]),
    labels: stringsIn(value["labels"]),
    types: numbersIn(value["types"], isItemType),
  };
}
