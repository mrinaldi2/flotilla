// A lead's proposed structure change, before and after (0.19.0): the
// board's columns as they are and as the change would leave them, each
// changed column marked with a glyph and a word, and the rules it adds or
// drops. Computed from the change the snapshot carries and the board as
// the owner sees it now.

import type { BoardColumn, ColumnKind } from "../../../protocol/gen/hermes/board/v1/board_pb";
import type {
  AutomationSet,
  BoardCreate,
  BoardProposal,
  BoardUpdate,
  ColumnAdd,
  ColumnRemove,
  ColumnUpdate,
} from "../../../protocol/gen/hermes/board/v1/requests_pb";
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

/** One line when `value` is set: "WIP limit 2". */
function when<T>(value: T | undefined, words: (value: T) => string): string[] {
  return value === undefined ? [] : [words(value)];
}

interface Board {
  readonly columns: readonly BoardColumn[];
  readonly before: readonly StripColumn[];
  readonly nameOf: (key: string) => string;
}

function columnAdd(b: Board, add: ColumnAdd): ProposalDiff {
  const column: StripColumn = {
    key: `+${add.name}`,
    name: add.name,
    kind: add.kind,
    mark: "added",
  };
  return {
    before: b.before,
    after: placed(b.before, column, add.after),
    rules: [
      ...add.rules.map((r) => `+ ${ruleWords(r)}`),
      ...(add.movers.length === 0 ? [] : [`Moved in by: ${moverWords(add.movers)}`]),
    ],
    notes: when(add.wipLimit, (n) => `WIP limit ${n}`),
  };
}

function policyDiff(b: Board, old: BoardColumn | undefined, update: ColumnUpdate): string[] {
  if (update.setPolicy !== true || old === undefined) {
    return [];
  }
  return [
    ...listDiff(old.rules, update.rules, ruleWords),
    ...listDiff(old.movers, update.movers, (t) => `mover: ${moverWords([t])}`),
    ...listDiff(old.allowedFrom, update.allowedFrom, (k) => `from ${b.nameOf(k)}`),
  ];
}

function columnUpdate(b: Board, update: ColumnUpdate): ProposalDiff {
  const old = b.columns.find((c) => c.key === update.key);
  const current = b.before.find((c) => c.key === update.key) ?? {
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
  const marked = b.before.map((c) => (c.key === update.key ? next : c));
  const after = update.after === undefined ? marked : placed(marked, next, update.after);
  return {
    before: b.before,
    after: update.visible === false ? after.filter((c) => c.key !== update.key) : after,
    rules: policyDiff(b, old, update),
    notes: [
      ...when(update.wipLimit, (n) => `WIP limit ${n}`),
      ...(update.clearWipLimit === true ? ["No WIP limit"] : []),
      ...when(update.visible, (shown) => (shown ? "Shown" : "Hidden")),
    ],
  };
}

function columnRemove(b: Board, remove: ColumnRemove): ProposalDiff {
  return {
    before: b.before,
    after: b.before.map((c) => (c.key === remove.key ? { ...c, mark: "removed" } : c)),
    rules: [],
    notes: when(remove.moveItemsTo, (to) => `Its items move to ${b.nameOf(to)}`),
  };
}

function automationSet(b: Board, set: AutomationSet): ProposalDiff {
  const to = set.columnKey ?? "";
  const note =
    to === ""
      ? `When ${factWord(set.fact)}: items stay where they are`
      : `When ${factWord(set.fact)} → move to ${b.nameOf(to)}`;
  return { before: b.before, after: b.before, rules: [], notes: [note] };
}

function boardCreate(create: BoardCreate): ProposalDiff {
  const preset = PRESETS.find((p) => p.name === create.preset);
  return {
    before: [],
    after: (preset?.columns ?? [])
      .filter((c) => c.hidden !== true)
      .map((c) => ({ key: c.name, name: c.name, kind: c.kind, mark: "added" })),
    rules: [],
    notes: [`A new board, ${create.name}, from the ${preset?.title ?? create.preset} preset`],
  };
}

function boardUpdate(b: Board, update: BoardUpdate): ProposalDiff {
  return {
    before: b.before,
    after: b.before,
    rules: [],
    notes: [
      ...when(update.name, (name) => `Renamed to ${name}`),
      ...when(update.description, () => "New description"),
    ],
  };
}

/** The change `proposal` makes to a board whose columns are `columns`. */
export function proposalDiff(
  proposal: BoardProposal,
  columns: readonly BoardColumn[],
): ProposalDiff {
  const before = strip(columns);
  const b: Board = {
    columns,
    before,
    nameOf: (key) => columns.find((c) => c.key === key)?.name ?? key,
  };
  const change = proposal.change;
  switch (change.case) {
    case "columnAdd":
      return columnAdd(b, change.value);
    case "columnUpdate":
      return columnUpdate(b, change.value);
    case "columnRemove":
      return columnRemove(b, change.value);
    case "automationSet":
      return automationSet(b, change.value);
    case "boardCreate":
      return boardCreate(change.value);
    case "boardUpdate":
      return boardUpdate(b, change.value);
    default:
      return { before, after: before, rules: [], notes: [proposal.summary] };
  }
}
