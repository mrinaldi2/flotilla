// Labels and a board's label groups (0.19.0 K8). A label `<group>:<value>`
// belongs to the group `<group>`; a board declares the groups it filters
// on, with the values it offers (`platform: ios, android`).

import type { Board, LabelGroup } from "../../protocol/gen/hermes/board/v1/board_pb";

/** A group's name and values, as the filter bar and the cards read them. */
export interface LabelGroupView {
  readonly name: string;
  readonly values: readonly string[];
}

/** The group a label belongs to, or "" for a plain label. */
export function labelGroupOf(label: string): string {
  const at = label.indexOf(":");
  return at > 0 && at < label.length - 1 ? label.slice(0, at) : "";
}

/** The label a group's value stands for. */
export function groupLabel(group: string, value: string): string {
  return `${group}:${value}`;
}

/** What a card shows for a label: a grouped one's value, else the label. */
export function labelWord(label: string): string {
  const group = labelGroupOf(label);
  return group === "" ? label : label.slice(group.length + 1);
}

/**
 * The most values a group the board doesn't declare may use and still be
 * offered. More is a tag of its own per card (an import's `was:B12`), which
 * would fill the bar with chips nobody filters on.
 */
const MAX_UNDECLARED_VALUES = 8;

/**
 * The groups the filter bar offers: the board's own, in order, then any
 * other group its cards use with at most `MAX_UNDECLARED_VALUES` values,
 * with the values they use.
 */
export function boardLabelGroups(
  board: Pick<Board, "labelGroups"> | undefined,
  cardLabels: readonly (readonly string[])[],
): readonly LabelGroupView[] {
  const declared: readonly LabelGroup[] = board?.labelGroups ?? [];
  const groups = new Map<string, string[]>(declared.map((g) => [g.name, [...g.values]]));
  for (const label of cardLabels.flat()) {
    const group = labelGroupOf(label);
    if (group === "") {
      continue;
    }
    const values = groups.get(group) ?? [];
    const value = labelWord(label);
    if (!values.includes(value)) {
      values.push(value);
    }
    groups.set(group, values);
  }
  const own = new Set(declared.map((g) => g.name));
  return [...groups]
    .filter(([name, values]) => own.has(name) || values.length <= MAX_UNDECLARED_VALUES)
    .map(([name, values]) => ({ name, values }));
}
