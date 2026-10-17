// The board's item drawer (U4): the open card's drawer, with ↑/↓ stepping
// through the cards its column shows and "Move to…" opening the board's own
// move menu, so a move from the drawer runs the same guards as a drag.
// Closing returns focus to the card of the item shown (UX-012). A draft
// (0.19.0) opens here too, from the drafts tray or "+ New item".

import type { ReactElement, ReactNode } from "react";
import type { DaemonApi } from "../../protocol/api";
import type { BoardApi } from "../../protocol/board";
import type { Bot } from "../../protocol/entities";
import type { BoardColumn, ItemCard } from "../../protocol/gen/flotilla/board/v1/board_pb";
import ItemDrawer from "./drawer/ItemDrawer";
import type { Anchor } from "./useBoardMoves";

interface BoardDrawerProps {
  readonly api: BoardApi;
  /** For a draft's docked chat. */
  readonly client?: DaemonApi;
  /** The board's project: its documents open in the drawer (0.20.0). */
  readonly projectId?: string;
  readonly openId: string | null;
  /** The cards the board shows, in board order. */
  readonly shown: readonly ItemCard[];
  /** The owner's drafts, which no column shows. */
  readonly drafts?: ReadonlyMap<string, ItemCard>;
  readonly columns: readonly BoardColumn[];
  readonly bots: readonly Bot[];
  readonly canControl: boolean;
  readonly connected?: boolean;
  readonly leadId?: string;
  readonly onOpen: (id: string | null) => void;
  readonly onMove: ((card: ItemCard, anchor: Anchor) => void) | null;
  readonly onDraftDone?: (message: string) => void;
  /** "Now", for ages; stories pin it. */
  readonly now?: number;
  /** Stories: the draft's chat docked at once. */
  readonly initialDiscuss?: string;
  /** The open item's commands for the owner (H-117), above its tabs. */
  readonly children?: ReactNode;
}

/** Focuses the board card of `id`, when the board shows it. */
function focusCard(id: string): void {
  const cards = document.querySelectorAll<HTMLElement>(".board-card[data-item-id]");
  Array.from(cards)
    .find((el) => el.dataset.itemId === id)
    ?.focus();
}

export default function BoardDrawer(props: BoardDrawerProps): ReactElement | null {
  const { openId, shown } = props;
  if (openId === null) {
    return null;
  }
  const card = shown.find((c) => c.id === openId);
  const draft = props.drafts?.get(openId);
  const column = shown.filter((c) => c.columnKey === card?.columnKey);
  const step = (direction: -1 | 1): void => {
    const next = column[column.findIndex((c) => c.id === openId) + direction];
    if (next !== undefined) {
      props.onOpen(next.id);
    }
  };
  const move = props.onMove;
  return (
    <ItemDrawer
      key={openId}
      api={props.api}
      client={props.client}
      projectId={props.projectId}
      itemId={openId}
      version={(card ?? draft)?.version}
      invited={draft?.invited}
      columns={props.columns}
      bots={props.bots}
      canComment={props.canControl}
      connected={props.connected}
      leadId={props.leadId}
      onClose={() => {
        props.onOpen(null);
        focusCard(openId);
      }}
      onMove={move && card ? (anchor) => move(card, anchor) : undefined}
      onStep={step}
      onDraftDone={props.onDraftDone}
      now={props.now === undefined ? undefined : () => props.now ?? 0}
      initialDiscuss={props.initialDiscuss}
    >
      {props.children}
    </ItemDrawer>
  );
}
