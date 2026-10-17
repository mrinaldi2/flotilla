// The owner's edit of an item from its drawer (0.19.0): the form's fields,
// and the `item_update` that carries only what changed. Acceptance
// criteria are one per line; labels are separated by commas.

import type { MessageInitShape } from "@bufbuild/protobuf";
import type { Item } from "../../../protocol/gen/flotilla/board/v1/board_pb";
import { ItemType, Priority, Size } from "../../../protocol/gen/flotilla/board/v1/board_pb";
import type { ItemUpdateSchema } from "../../../protocol/gen/flotilla/board/v1/requests_pb";
import { ITEM_TYPES_V2 } from "../../../protocol/plan";

export interface ItemForm {
  readonly title: string;
  readonly description: string;
  readonly type: ItemType;
  readonly priority: Priority;
  /** Unset stays unset: an edit can set a size, not clear one. */
  readonly size: Size | undefined;
  readonly labels: string;
  readonly criteria: string;
  /** A bot id, or "" for unassigned. */
  readonly assignee: string;
}

export function formOf(item: Item): ItemForm {
  return {
    title: item.title,
    description: item.description,
    type: item.type,
    priority: item.priority,
    size: item.size,
    labels: item.labels.join(", "),
    criteria: item.acceptanceCriteria.map((c) => c.text).join("\n"),
    assignee: item.assignee ?? "",
  };
}

function list(text: string, by: string): string[] {
  return text
    .split(by)
    .map((part) => part.trim())
    .filter(Boolean);
}

function same(a: readonly string[], b: readonly string[]): boolean {
  return a.length === b.length && a.every((value, i) => value === b[i]);
}

/** Why the form can't be saved, or null. */
export function formProblem(form: ItemForm): string | null {
  if (form.title.trim() === "") {
    return "Give it a title.";
  }
  const criteria = list(form.criteria, "\n");
  if (new Set(criteria).size !== criteria.length) {
    return "Two acceptance criteria say the same thing.";
  }
  return null;
}

/** The `item_update` for what changed, or null when nothing did. */
export function updateFor(
  item: Item,
  form: ItemForm,
): MessageInitShape<typeof ItemUpdateSchema> | null {
  const title = form.title.trim();
  const labels = list(form.labels, ",");
  const criteria = list(form.criteria, "\n");
  const changes = {
    ...(title === item.title ? {} : { title }),
    ...(form.description === item.description ? {} : { description: form.description }),
    ...(form.type === item.type ? {} : { type: form.type }),
    ...(form.priority === item.priority ? {} : { priority: form.priority }),
    ...(form.size === undefined || form.size === item.size ? {} : { size: form.size }),
    ...(same(labels, item.labels) ? {} : { labels: { values: labels } }),
    ...(same(
      criteria,
      item.acceptanceCriteria.map((c) => c.text),
    )
      ? {}
      : { acceptanceCriteria: { values: criteria } }),
  };
  if (Object.keys(changes).length === 0) {
    return null;
  }
  return { id: item.id, expectedVersion: item.version, ...changes };
}

/**
 * The card types (0.21.0, #43), Task first: features and epics are created
 * in the Plan, and chores became tasks.
 */
const TYPES: readonly ItemType[] = [ItemType.TASK, ItemType.DESIGN, ItemType.BUG, ItemType.SPIKE];

/** What a Flotilla service before 0.21 takes; it refuses Task and Design. */
const OLDER_TYPES: readonly ItemType[] = [
  ItemType.FEATURE,
  ItemType.BUG,
  ItemType.CHORE,
  ItemType.SPIKE,
  ItemType.EPIC,
];

/** The types a new card can take on this service; the first is the default. */
export function typesFor(capabilities: readonly string[]): readonly ItemType[] {
  return capabilities.includes(ITEM_TYPES_V2) ? TYPES : OLDER_TYPES;
}

/** The edit form's types: the service's, and the card's own when it is a retired one. */
export function editTypes(capabilities: readonly string[], current: ItemType): readonly ItemType[] {
  const types = typesFor(capabilities);
  return types.includes(current) ? types : [...types, current];
}
export const PRIORITIES: readonly Priority[] = [Priority.P0, Priority.P1, Priority.P2, Priority.P3];
export const SIZES: readonly Size[] = [Size.S, Size.M, Size.L];
