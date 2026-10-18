import { MoreHorizontal } from "lucide-react";
import type { DragEvent, KeyboardEvent, ReactElement } from "react";
import type { Bot } from "../../protocol/entities";
import type { ItemCard } from "../../protocol/gen/flotilla/board/v1/board_pb";
import type { PlanNode } from "../../protocol/gen/flotilla/board/v1/plan_pb";
import { kindWords } from "../plan/planText";
import BotAvatar from "../BotAvatar";
import { labelGroupOf, labelWord } from "./labelGroups";
import { typeLabel, urgentPriority } from "./labels";
import { stuckLine } from "./moves";
import type { Anchor } from "./useBoardMoves";

/** The label the daemon adds when the lead overrode a WIP limit. */
const WIP_OVERRIDE_LABEL = "wip-override";

interface BoardCardProps {
  readonly card: ItemCard;
  readonly columnName: string;
  readonly assignee: Bot | undefined;
  /** Null when this connection can't move items. */
  readonly onOpenMoveMenu: ((card: ItemCard, anchor: Anchor) => void) | null;
  /** Opens the item drawer (U4): a click, or Enter on the focused card. */
  readonly onOpen?: (id: string) => void;
  readonly onDragStart: (card: ItemCard) => void;
  readonly onDragEnd: () => void;
  readonly dragging: boolean;
  /** The label groups whose labels show as chips: the board's own. */
  readonly chipGroups?: ReadonlySet<string>;
  /** A column's name by key, for the "can't move" line. */
  readonly columnNameOf?: (key: string) => string;
  /** The plan node it belongs to (0.21.0), shown as a chip. */
  readonly node?: PlanNode;
}

/** "E Hit feedback": the plan node a card belongs to, its kind as a glyph. */
function NodeChip({ node }: { readonly node: PlanNode }): ReactElement {
  const kind = kindWords(node.kind);
  return (
    <span className="board-card-node" title={`${kind.word}: ${node.title}`}>
      <span className={`plan-glyph plan-glyph-${kind.word.toLowerCase()}`} aria-hidden="true">
        {kind.glyph}
      </span>
      <span className="visually-hidden">{`${kind.word}: `}</span>
      <span className="board-card-node-title">{node.title}</span>
    </span>
  );
}

function assigneeName(card: ItemCard, assignee: Bot | undefined): string | null {
  return assignee?.name ?? card.assignee ?? null;
}

/** "H-017, Feature, Board and dashboard, Doing, Desktop Dev, blocked, stale" (H-018 §3.2). */
function cardAccessibleName(
  card: ItemCard,
  columnName: string,
  assignee: Bot | undefined,
  stuck: string | null,
): string {
  const parts = [card.id, typeLabel(card.type).word, card.title, columnName];
  parts.push(assigneeName(card, assignee) ?? "unassigned");
  const priority = urgentPriority(card.priority);
  if (priority !== null) {
    parts.push(`priority ${priority}`);
  }
  if (card.blocked) {
    parts.push("blocked");
  }
  if (card.stale) {
    parts.push("stale");
  }
  if (stuck !== null) {
    parts.push(stuck);
  }
  return parts.join(", ");
}

function anchorBelow(element: Element): Anchor {
  const rect = element.getBoundingClientRect();
  return { x: rect.left, y: rect.bottom + 4 };
}

/** The card's status lines: blocked, over WIP, can't move, criteria. Each is left out when it has nothing to say. */
function CardLines({
  card,
  stuck,
}: {
  readonly card: ItemCard;
  readonly stuck: string | null;
}): ReactElement {
  const overWip = card.labels.includes(WIP_OVERRIDE_LABEL);
  const why =
    card.moveBlock?.refused === true
      ? "The last move was refused."
      : "The next column's rules stop it moving on.";
  return (
    <>
      {card.blocked ? (
        <div className="board-card-line board-card-blocked-line">
          <span aria-hidden="true">⛔</span> Blocked
        </div>
      ) : null}
      {overWip ? (
        <div className="board-card-line board-card-override">
          <span aria-hidden="true">⚑</span> Over WIP
        </div>
      ) : null}
      {stuck === null ? null : (
        <div className="board-card-line board-card-stuck" title={why}>
          <span aria-hidden="true">⚠</span> {stuck}
        </div>
      )}
      {card.acTotal > 0 ? (
        <div className="board-card-line board-card-meta">
          <span aria-hidden="true">☑</span> {card.acChecked}/{card.acTotal} AC
        </div>
      ) : null}
    </>
  );
}

/** One item on the board: at most seven lines, each omitted when it has nothing to say. */
export default function BoardCard(props: BoardCardProps): ReactElement {
  const { card, columnName, assignee, onOpenMoveMenu, dragging } = props;
  const type = typeLabel(card.type);
  const priority = urgentPriority(card.priority);
  const name = assigneeName(card, assignee);
  const block = card.moveBlock;
  const stuck =
    block === undefined
      ? null
      : stuckLine(block, props.columnNameOf?.(block.toColumn) ?? block.toColumn);

  const onKeyDown = (event: KeyboardEvent<HTMLElement>): void => {
    if (props.onOpen && event.target === event.currentTarget && event.key === "Enter") {
      event.preventDefault();
      props.onOpen(card.id);
      return;
    }
    if (
      onOpenMoveMenu !== null &&
      event.target === event.currentTarget &&
      (event.key === "m" || event.key === "M") &&
      !event.metaKey &&
      !event.ctrlKey &&
      !event.altKey
    ) {
      event.preventDefault();
      onOpenMoveMenu(card, anchorBelow(event.currentTarget));
    }
  };

  const chips = card.labels.filter((label) => props.chipGroups?.has(labelGroupOf(label)) ?? false);

  const onDragStart = (event: DragEvent<HTMLElement>): void => {
    event.dataTransfer.effectAllowed = "move";
    event.dataTransfer.setData("text/plain", card.id);
    props.onDragStart(card);
  };

  const className = [
    "board-card",
    card.blocked ? "board-card-blocked" : "",
    dragging ? "board-card-dragging" : "",
  ]
    .filter(Boolean)
    .join(" ");

  // H-018 §3.2: a card is a focusable <article>; M opens "Move to…" on it.
  return (
    // oxlint-disable-next-line jsx-a11y/no-noninteractive-element-interactions, jsx-a11y/no-noninteractive-tabindex
    <article
      className={className}
      // oxlint-disable-next-line jsx-a11y/no-noninteractive-tabindex
      tabIndex={0}
      aria-label={cardAccessibleName(card, columnName, assignee, stuck)}
      data-item-id={card.id}
      draggable={onOpenMoveMenu !== null}
      onDragStart={onDragStart}
      onDragEnd={props.onDragEnd}
      onKeyDown={onKeyDown}
      onClick={(event) => {
        // The card's own buttons (Move to…) act on their own.
        if (!(event.target instanceof Element && event.target.closest("button"))) {
          props.onOpen?.(card.id);
        }
      }}
    >
      <div className="board-card-top">
        <span className="board-card-type">
          <span aria-hidden="true">{type.glyph}</span> {type.word}
        </span>
        <span className="board-card-sep" aria-hidden="true">
          ·
        </span>
        <span className="board-card-id">{card.id}</span>
        {priority === null ? null : (
          <span className={`board-card-priority board-card-${priority.toLowerCase()}`}>
            <span aria-hidden="true">‼</span> {priority}
          </span>
        )}
        {onOpenMoveMenu === null ? null : (
          <button
            type="button"
            className="board-card-more"
            aria-label={`Move ${card.id} to…`}
            title="Move to… (M)"
            aria-haspopup="menu"
            tabIndex={-1}
            onClick={(event) => {
              const article = event.currentTarget.closest("article");
              onOpenMoveMenu(card, anchorBelow(article ?? event.currentTarget));
            }}
          >
            <MoreHorizontal size={14} aria-hidden="true" />
          </button>
        )}
      </div>
      <div className="board-card-title">{card.title}</div>
      {chips.length === 0 ? null : (
        <div className="board-card-chips">
          {chips.map((label) => (
            <span key={label} className="board-chip" title={label}>
              {labelWord(label)}
            </span>
          ))}
        </div>
      )}
      {props.node === undefined ? null : <NodeChip node={props.node} />}
      <CardLines card={card} stuck={stuck} />
      <div className="board-card-foot">
        {name === null ? (
          <span className="board-card-unassigned">Unassigned</span>
        ) : (
          <span className="board-card-assignee">
            <BotAvatar
              avatar={assignee?.avatar ?? ""}
              name={name}
              id={card.assignee ?? name}
              size="sm"
            />
            <span className="board-card-assignee-name">{name}</span>
          </span>
        )}
        {card.stale ? (
          <span className="board-card-stale">
            <span aria-hidden="true">⏱</span> Stale
          </span>
        ) : null}
      </div>
    </article>
  );
}
