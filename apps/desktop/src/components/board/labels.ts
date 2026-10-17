// Owner-facing words for the board's enums (ux-glossary §5).
// Every type carries a glyph and a word, so nothing depends on colour.

import { ItemType, Priority } from "../../protocol/gen/flotilla/board/v1/board_pb";

interface TypeLabel {
  readonly glyph: string;
  readonly word: string;
}

const TYPE_LABEL: Readonly<Record<ItemType, TypeLabel>> = {
  [ItemType.UNSPECIFIED]: { glyph: "•", word: "Item" },
  [ItemType.EPIC]: { glyph: "◆", word: "Epic" },
  [ItemType.FEATURE]: { glyph: "▲", word: "Feature" },
  [ItemType.BUG]: { glyph: "✱", word: "Bug" },
  [ItemType.SPIKE]: { glyph: "◎", word: "Spike" },
  [ItemType.CHORE]: { glyph: "▪", word: "Chore" },
  [ItemType.TASK]: { glyph: "■", word: "Task" },
  [ItemType.DESIGN]: { glyph: "✎", word: "Design" },
};

export function typeLabel(type: ItemType): TypeLabel {
  return TYPE_LABEL[type] ?? TYPE_LABEL[ItemType.UNSPECIFIED];
}

const TYPE_HINT: Readonly<Partial<Record<ItemType, string>>> = {
  [ItemType.TASK]: "Build something. Passes Research first.",
  [ItemType.DESIGN]: "Mockups or a spec. Attaches to tasks when done.",
  [ItemType.BUG]: "Something broken. Passes Research first.",
  [ItemType.SPIKE]: "A timeboxed question that ends in a document.",
};

/** What a card type is for, in one line; "" for the retired types. */
export function typeHint(type: ItemType): string {
  return TYPE_HINT[type] ?? "";
}

/** The types the Type filter offers: the card types (0.21.0); features and epics are in the Plan. */
export const FILTER_TYPES: readonly ItemType[] = [
  ItemType.TASK,
  ItemType.DESIGN,
  ItemType.BUG,
  ItemType.SPIKE,
];

/** "P0" / "P1" for the priorities a card calls out; the rest stay quiet. */
export function urgentPriority(priority: Priority): string | null {
  if (priority === Priority.P0) {
    return "P0";
  }
  if (priority === Priority.P1) {
    return "P1";
  }
  return null;
}
