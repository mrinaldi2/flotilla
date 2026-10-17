// One board's columns and cards, with drag and drop (H-018 §3): a drag
// checks the card's moves, each column shows whether it takes it, and the
// drop asks the move engine. Inbox folds into a rail on a release-gated
// board, where it is triage; a custom board's open columns are its flow.

import { useState } from "react";
import type { ReactElement } from "react";
import type { Bot } from "../../protocol/entities";
import type { BoardColumn, ItemCard } from "../../protocol/gen/flotilla/board/v1/board_pb";
import type { PlanNode } from "../../protocol/gen/flotilla/board/v1/plan_pb";
import { ColumnCategory, ColumnKind } from "../../protocol/gen/flotilla/board/v1/board_pb";
import BoardCard from "./BoardCard";
import BoardColumnView, { InboxRail } from "./BoardColumnView";
import type { DropState } from "./BoardColumnView";
import type { BoardPrefs } from "./boardPrefs";
import type { NodeScope } from "./filters";
import { matchesFilters } from "./filters";
import type { ColumnChecks } from "./moves";
import { planMove, reasonChip } from "./moves";
import type { Anchor, BoardMoves } from "./useBoardMoves";
import { wipSummary } from "./wip";

interface BoardColumnsProps {
  readonly columns: readonly BoardColumn[];
  /** The board's cards, in rank order. */
  readonly cards: readonly ItemCard[];
  readonly prefs: BoardPrefs;
  readonly onPrefs: (prefs: BoardPrefs) => void;
  /** A release-gated board (or one from a home before 0.19): Inbox is a rail. */
  readonly gated: boolean;
  readonly botsById: ReadonlyMap<string, Bot>;
  readonly moves: BoardMoves;
  readonly canMove: boolean;
  readonly onOpen: (id: string) => void;
  readonly onSetLimit?: (column: BoardColumn) => (limit: number | undefined) => Promise<void>;
  /** "+ New item" at the foot of open columns. */
  readonly onNewItem?: () => void;
  /** The label groups whose labels show on the cards. */
  readonly chipGroups?: ReadonlySet<string>;
  /** The plan (0.21.0): each node and those under it, for the node filter. */
  readonly scope?: NodeScope;
  /** The plan's nodes, for each card's node chip. */
  readonly nodesById?: ReadonlyMap<string, PlanNode>;
}

interface Drag {
  readonly card: ItemCard;
  /** Null until the move check answers. */
  readonly checks: ColumnChecks | null;
}

function dropStateFor(column: BoardColumn, drag: Drag | null): DropState {
  if (drag === null) {
    return { kind: "idle" };
  }
  if (column.key === drag.card.columnKey || drag.checks === null) {
    return { kind: "allowed" };
  }
  const unmet = drag.checks.get(column.key) ?? [];
  return planMove(unmet).kind === "refused"
    ? { kind: "refused", chip: reasonChip(unmet) }
    : { kind: "allowed" };
}

export default function BoardColumns(props: BoardColumnsProps): ReactElement {
  const { cards, prefs, moves, botsById } = props;
  const [drag, setDrag] = useState<Drag | null>(null);
  const [announce, setAnnounce] = useState("");
  const columns = props.columns.filter((column) => column.visible || prefs.showHidden);
  const columnNameOf = (key: string): string =>
    props.columns.find((column) => column.key === key)?.name ?? key;

  const onDragStart = (card: ItemCard): void => {
    setDrag({ card, checks: null });
    void (async () => {
      try {
        const checks = await moves.checksFor(card);
        setDrag((current) => (current?.card.id === card.id ? { card, checks } : current));
      } catch {
        // The drop runs the check again and reports the failure.
      }
    })();
  };

  const onDrop = (column: BoardColumn, anchor: Anchor): void => {
    const current = drag;
    setDrag(null);
    setAnnounce("");
    if (current !== null) {
      moves.requestMove(current.card, column.key, anchor);
    }
  };

  return (
    <>
      <div className={`board-columns${drag === null ? "" : " board-dragging"}`}>
        {columns.map((column) => {
          const inColumn = cards.filter((card) => card.columnKey === column.key);
          const visible = inColumn.filter((card) =>
            matchesFilters(card, prefs.filters, props.scope),
          );
          const summary = wipSummary(column, inColumn, visible.length);
          const drop = dropStateFor(column, drag);
          const handlers = {
            onDragEnter: () => {
              setAnnounce(drop.kind === "refused" ? `${column.name}: ${drop.chip}` : "");
            },
            onDrop: (anchor: Anchor) => {
              onDrop(column, anchor);
            },
          };
          const inbox = props.gated && column.category === ColumnCategory.INBOX;
          if (inbox && !prefs.inboxOpen) {
            return (
              <InboxRail
                key={column.key}
                column={column}
                summary={summary}
                drop={drop}
                onExpand={() => props.onPrefs({ ...prefs, inboxOpen: true })}
                {...handlers}
              />
            );
          }
          return (
            <BoardColumnView
              key={column.key}
              column={column}
              summary={summary}
              drop={drop}
              botsById={botsById}
              onSetLimit={props.onSetLimit?.(column)}
              onCollapse={inbox ? () => props.onPrefs({ ...prefs, inboxOpen: false }) : undefined}
              onNewItem={column.kind === ColumnKind.OPEN ? props.onNewItem : undefined}
              {...handlers}
            >
              {visible.map((card) => (
                <BoardCard
                  key={card.id}
                  card={card}
                  columnName={column.name}
                  assignee={card.assignee === undefined ? undefined : botsById.get(card.assignee)}
                  onOpenMoveMenu={props.canMove ? moves.openMenu : null}
                  onOpen={props.onOpen}
                  onDragStart={onDragStart}
                  onDragEnd={() => {
                    setDrag(null);
                    setAnnounce("");
                  }}
                  dragging={drag?.card.id === card.id}
                  chipGroups={props.chipGroups}
                  columnNameOf={columnNameOf}
                  node={
                    card.planNodeId === undefined
                      ? undefined
                      : props.nodesById?.get(card.planNodeId)
                  }
                />
              ))}
            </BoardColumnView>
          );
        })}
      </div>
      <div className="visually-hidden" aria-live="polite">
        {announce}
      </div>
    </>
  );
}
