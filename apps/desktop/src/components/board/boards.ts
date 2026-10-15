// A project's boards (0.19.0): which one the Board tab shows, what the
// switcher lists, and the columns, cards and drafts of one board. A home
// before 0.19 sends no boards: then the project has one, implied, and every
// column and card is on it.

import type { Board, BoardColumn, ItemCard } from "../../protocol/gen/flotilla/board/v1/board_pb";
import type { BoardModel } from "./boardSync";

function byPlace(a: Board, b: Board): number {
  if (a.isDefault !== b.isDefault) {
    return a.isDefault ? -1 : 1;
  }
  return a.ord - b.ord || a.name.localeCompare(b.name);
}

/** The boards the switcher lists: the default first, then the others by `ord`. */
export function liveBoards(boards: readonly Board[]): Board[] {
  const live = boards.filter((board) => !board.archived);
  // oxlint-disable-next-line unicorn/no-array-sort
  live.sort(byPlace);
  return live;
}

/** Archived boards, by name. */
export function archivedBoards(boards: readonly Board[]): Board[] {
  const archived = boards.filter((board) => board.archived);
  // oxlint-disable-next-line unicorn/no-array-sort
  archived.sort((a, b) => a.name.localeCompare(b.name));
  return archived;
}

/** The board an item is on, from the board's cards or the owner's drafts. */
export function boardOfItem(model: BoardModel, itemId: string): string | undefined {
  const card = model.cards.get(itemId) ?? model.drafts.get(itemId);
  return card === undefined || card.boardId === "" ? undefined : card.boardId;
}

interface Wanted {
  /** Asked for by a link or the selection. */
  readonly asked?: string;
  /** The item a link opens: its own board wins. */
  readonly item?: string;
  /** The board last open in this project. */
  readonly remembered?: string;
}

/**
 * The board the tab shows: the linked item's own board, then the one asked
 * for, then the one last open, then the default. Archived boards show only
 * when asked for. `undefined` when the home sends no boards.
 */
export function pickBoard(model: BoardModel, wanted: Wanted): Board | undefined {
  const byId = (id: string | undefined): Board | undefined =>
    id === undefined ? undefined : model.boards.find((board) => board.id === id);
  const live = liveBoards(model.boards);
  const remembered = byId(wanted.remembered);
  return (
    byId(wanted.item === undefined ? undefined : boardOfItem(model, wanted.item)) ??
    byId(wanted.asked) ??
    (remembered?.archived === false ? remembered : undefined) ??
    live[0] ??
    model.boards[0]
  );
}

/** Whether a column or card belongs to `board` (everything, without boards). */
function onBoard(boardId: string, board: Board | undefined): boolean {
  return board === undefined || boardId === "" || boardId === board.id;
}

export function columnsOf(model: BoardModel, board: Board | undefined): BoardColumn[] {
  return model.columns.filter((column) => onBoard(column.boardId, board));
}

export function cardsOf(cards: readonly ItemCard[], board: Board | undefined): ItemCard[] {
  return cards.filter((card) => onBoard(card.boardId, board));
}

/** The item-id prefix a name suggests: its initials in capitals, as the Flotilla service picks. */
export function suggestKey(name: string): string {
  const initials = name
    .split(/[^\p{L}\p{N}]+/u)
    .map((word) => word.charAt(0))
    .filter((c) => /[A-Za-z]/.test(c))
    .map((c) => c.toUpperCase())
    .slice(0, 4)
    .join("");
  return initials === "" ? "B" : initials;
}

/** Why a key can't be used, or null: capitals and digits, unique across the project's boards. */
export function keyProblem(key: string, boards: readonly Board[]): string | null {
  if (key === "") {
    return "Give the board a key.";
  }
  if (!/^[A-Z][A-Z0-9]{0,7}$/.test(key)) {
    return "Use 1–8 capital letters or digits, starting with a letter.";
  }
  const taken = boards.find((board) => board.key === key);
  return taken === undefined ? null : `${key} is already the key of ${taken.name}.`;
}
