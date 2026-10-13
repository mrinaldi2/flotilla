// A lead's proposed structure change, before and after (0.19.0): the
// board's columns as they are and as the change would leave them, each
// changed column marked with a glyph and a word, and the rules it adds or
// drops. Computed from the change the snapshot carries and the board as
// the owner sees it now.

import type { BoardColumn, ColumnKind } from "../../../protocol/gen/hermes/board/v1/board_pb";
import type { BoardProposal } from "../../../protocol/gen/hermes/board/v1/requests_pb";
import { PRESETS } from "../presets";
import { factWord, moverWords, ruleWords } from "../settings/rules";

export interface StripColumn {
  readonly key: string;
  readonly name: string;
  readonly kind: ColumnKind;
  /** "added", "removed" or "changed"; unset when the change leaves it be. */
  readonly mark?: "added" | "removed" | "changed";
}

export interface ProposalDiff {
  readonly before: readonly StripColumn[];
  readonly after: readonly StripColumn[];
  /** The rule changes, one line each: "+ Has an assignee", "− Size at most M". */
  readonly rules: readonly string[];
  /** Anything else it changes, in words. */
  readonly notes: readonly string[];
}

function strip(columns: readonly BoardColumn[]): StripColumn[] {
  return columns.filter((c) => c.visible).map((c) => ({ key: c.key, name: c.name, kind: c.kind }));
}

/** `column` placed after `after` ("" first; unset or unknown, last). */
function placed(
  columns: readonly StripColumn[],
  column: StripColumn,
  after: string | undefined,
): StripColumn[] {
  const rest = columns.filter((c) => c.key !== column.key);
  if (after === "") {
    return [column, ...rest];
  }
  const at = after === undefined ? -1 : rest.findIndex((c) => c.key === after);
  return at < 0 ? [...rest, column] : [...rest.slice(0, at + 1), column, ...rest.slice(at + 1)];
}

function listDiff(
  before: readonly string[],
  after: readonly string[],
  words: (t: string) => string,
): string[] {
  return [
    ...after.filter((t) => !before.includes(t)).map((t) => `+ ${words(t)}`),
    ...before.filter((t) => !after.includes(t)).map((t) => `− ${words(t)}`),
  ];
}

/** The change `proposal` makes to a board whose columns are `columns`. */
export function proposalDiff(
  proposal: BoardProposal,
  columns: readonly BoardColumn[],
): ProposalDiff {
  const before = strip(columns);
  const change = proposal.change;
  const nameOf = (key: string): string => columns.find((c) => c.key === key)?.name ?? key;
  switch (change.case) {
    case "columnAdd": {
      const add = change.value;
      const column: StripColumn = {
        key: `+${add.name}`,
        name: add.name,
        kind: add.kind,
        mark: "added",
      };
      return {
        before,
        after: placed(before, column, add.after),
        rules: [
          ...add.rules.map((r) => `+ ${ruleWords(r)}`),
          ...(add.movers.length === 0 ? [] : [`Moved in by: ${moverWords(add.movers)}`]),
        ],
        notes: add.wipLimit === undefined ? [] : [`WIP limit ${add.wipLimit}`],
      };
    }
    case "columnUpdate": {
      const update = change.value;
      const old = columns.find((c) => c.key === update.key);
      const current = before.find((c) => c.key === update.key) ?? {
        key: update.key,
        name: old?.name ?? update.key,
        kind: old?.kind ?? 0,
      };
      const next: StripColumn = {
        ...current,
        name: update.name ?? current.name,
        kind: update.kind ?? current.kind,
        mark: "changed",
      };
      const marked = before.map((c) => (c.key === update.key ? next : c));
      const after = update.after === undefined ? marked : placed(marked, next, update.after);
      const policy = update.setPolicy === true && old !== undefined;
      return {
        before,
        after: update.visible === false ? after.filter((c) => c.key !== update.key) : after,
        rules: policy
          ? [
              ...listDiff(old.rules, update.rules, ruleWords),
              ...listDiff(old.movers, update.movers, (t) => `mover: ${moverWords([t])}`),
              ...listDiff(old.allowedFrom, update.allowedFrom, (k) => `from ${nameOf(k)}`),
            ]
          : [],
        notes: [
          ...(update.wipLimit === undefined ? [] : [`WIP limit ${update.wipLimit}`]),
          ...(update.clearWipLimit === true ? ["No WIP limit"] : []),
          ...(update.visible === undefined ? [] : [update.visible ? "Shown" : "Hidden"]),
        ],
      };
    }
    case "columnRemove": {
      const remove = change.value;
      return {
        before,
        after: before.map((c) => (c.key === remove.key ? { ...c, mark: "removed" } : c)),
        rules: [],
        notes:
          remove.moveItemsTo === undefined
            ? []
            : [`Its items move to ${nameOf(remove.moveItemsTo)}`],
      };
    }
    case "automationSet": {
      const set = change.value;
      return {
        before,
        after: before,
        rules: [],
        notes: [
          set.columnKey === undefined || set.columnKey === ""
            ? `When ${factWord(set.fact)}: items stay where they are`
            : `When ${factWord(set.fact)} → move to ${nameOf(set.columnKey)}`,
        ],
      };
    }
    case "boardCreate": {
      const preset = PRESETS.find((p) => p.name === change.value.preset);
      return {
        before: [],
        after: (preset?.columns ?? [])
          .filter((c) => c.hidden !== true)
          .map((c) => ({ key: c.name, name: c.name, kind: c.kind, mark: "added" })),
        rules: [],
        notes: [
          `A new board, ${change.value.name}, from the ${preset?.title ?? change.value.preset} preset`,
        ],
      };
    }
    case "boardUpdate": {
      const update = change.value;
      return {
        before,
        after: before,
        rules: [],
        notes: [
          ...(update.name === undefined ? [] : [`Renamed to ${update.name}`]),
          ...(update.description === undefined ? [] : ["New description"]),
        ],
      };
    }
    default:
      return { before, after: before, rules: [], notes: [proposal.summary] };
  }
}
