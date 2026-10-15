// The presets a new board starts from (kanban/presets.rs), as the New board
// sheet previews them. Flotilla service builds the board; this is only
// what the owner sees before choosing.

import { ColumnKind } from "../../protocol/gen/flotilla/board/v1/board_pb";

export type PresetName = "simple" | "simple_review" | "software" | "release_gated";

interface PreviewColumn {
  readonly name: string;
  readonly kind: ColumnKind;
  /** Rules in words, for the preview's second line. */
  readonly note?: string;
  readonly hidden?: boolean;
}

export interface Preset {
  readonly name: PresetName;
  readonly title: string;
  readonly blurb: string;
  readonly columns: readonly PreviewColumn[];
}

const { OPEN, ACTIVE, DONE, CANCELLED } = ColumnKind;
const cancelled: PreviewColumn = { name: "Cancelled", kind: CANCELLED, hidden: true };

export const PRESETS: readonly Preset[] = [
  {
    name: "simple",
    title: "Simple",
    blurb: "To do, Doing and Done. Anyone moves items; Doing holds 3.",
    columns: [
      { name: "To do", kind: OPEN },
      { name: "Doing", kind: ACTIVE, note: "WIP 3" },
      { name: "Done", kind: DONE },
      cancelled,
    ],
  },
  {
    name: "simple_review",
    title: "Simple + Review",
    blurb:
      "A new project's first board. The bot works a card and moves it to Review; you move it to Done.",
    columns: [
      { name: "To do", kind: OPEN, note: "assignee or lead" },
      { name: "Doing", kind: ACTIVE, note: "assignee set · WIP 3 · starts with the work" },
      { name: "Review", kind: ACTIVE, note: "from Doing" },
      { name: "Done", kind: DONE, note: "you, or a lead who didn't do it" },
      cancelled,
    ],
  },
  {
    name: "software",
    title: "Software",
    blurb: "Refined before work starts; a PR moves items to Review and Done.",
    columns: [
      { name: "Inbox", kind: OPEN },
      { name: "Ready", kind: OPEN, note: "lead · criteria, size ≤ M" },
      { name: "Doing", kind: ACTIVE, note: "assignee set · WIP 1 each" },
      { name: "Review", kind: ACTIVE, note: "from Doing · PR linked" },
      { name: "Done", kind: DONE, note: "lead" },
      cancelled,
    ],
  },
  {
    name: "release_gated",
    title: "Release-gated",
    blurb: "The built-in flow of 0.18: items reach Done through a release.",
    columns: [
      { name: "Inbox", kind: OPEN },
      { name: "Ready", kind: OPEN },
      { name: "Doing", kind: ACTIVE },
      { name: "Review", kind: ACTIVE },
      { name: "Verify", kind: ACTIVE },
      { name: "Awaiting owner", kind: ACTIVE },
      { name: "Deploying", kind: ACTIVE },
      { name: "Done", kind: DONE },
      cancelled,
    ],
  },
];
