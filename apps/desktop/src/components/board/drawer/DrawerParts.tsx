// The item drawer's fixed parts: the header with Move to…, the
// state stepper, the meta line and the next step.

import { Pencil, X } from "lucide-react";
import type { ReactElement } from "react";
import type { BoardColumn, Item } from "../../../protocol/gen/hermes/board/v1/board_pb";
import { Priority, Size } from "../../../protocol/gen/hermes/board/v1/board_pb";
import { typeLabel } from "../labels";
import type { Anchor } from "../useBoardMoves";
import type { NextStep } from "./drawerText";
import { age, flowOf } from "./drawerText";

export function DrawerHeader(props: {
  readonly itemId: string;
  readonly item: Item | undefined;
  /** A draft: "Draft · RB-20 · not on the board yet". */
  readonly draft?: boolean;
  readonly onMove?: (anchor: Anchor) => void;
  /** The owner's edit form; absent where the item can't be edited. */
  readonly onEdit?: () => void;
  readonly onClose: () => void;
  /** The card shown before this one, when the drawer has one (UX-035 §4). */
  readonly back?: { readonly id: string; readonly onBack: () => void };
}): ReactElement {
  const type = props.item ? typeLabel(props.item.type) : null;
  const move = props.onMove;
  const back = props.back;
  return (
    <header className="drawer-head">
      {back ? (
        <button
          type="button"
          className="btn btn-small drawer-back"
          aria-label={`Back to ${back.id}`}
          onClick={back.onBack}
        >
          ‹ Back
        </button>
      ) : null}
      {props.draft === true ? (
        <span className="drawer-type drawer-draft-head">
          <span aria-hidden="true">✎ </span>
          <b>Draft</b> · <span className="mono">{props.itemId}</span> · not on the board yet
        </span>
      ) : (
        <span className="drawer-type">
          {type ? `${type.glyph} ${type.word}` : ""} · <span className="mono">{props.itemId}</span>
        </span>
      )}
      {props.onEdit ? (
        <button type="button" className="btn btn-small" onClick={props.onEdit}>
          <Pencil size={12} aria-hidden="true" /> Edit
        </button>
      ) : null}
      {move ? (
        <button
          type="button"
          className="btn btn-small"
          onClick={(event) => {
            const rect = event.currentTarget.getBoundingClientRect();
            move({ x: rect.left, y: rect.bottom + 4 });
          }}
        >
          Move to…
        </button>
      ) : null}
      <button
        type="button"
        className="drawer-close"
        aria-label="Close the item"
        onClick={props.onClose}
      >
        <X size={16} aria-hidden="true" />
      </button>
    </header>
  );
}

function stepGlyph(index: number, reached: number): string {
  if (index === reached) {
    return "◉";
  }
  return index < reached ? "●" : "○";
}

/** The item's path, the current step filled; words, not colour. */
export function Stepper(props: {
  readonly columns: readonly BoardColumn[];
  readonly column: BoardColumn | undefined;
}): ReactElement {
  const { steps, reached } = flowOf(props.columns, props.column);
  return (
    <ol className="drawer-stepper" aria-label="Where it stands">
      {steps.map((s, index) => (
        <li
          key={s.key}
          className={index === reached ? "now" : index < reached ? "past" : ""}
          aria-current={index === reached ? "step" : undefined}
        >
          <span aria-hidden="true">{stepGlyph(index, reached)}</span> {s.word}
        </li>
      ))}
    </ol>
  );
}

/** "Doing for 31h · Lead · P1 · M · area:web, urgent". */
export function MetaLine(props: {
  readonly item: Item;
  readonly column: BoardColumn | undefined;
  readonly who: (actor: string) => string;
  readonly now: number;
}): ReactElement {
  const { item, column } = props;
  const parts = [
    column ? `${column.name} for ${age(item.stateEnteredAt, props.now)}` : item.columnKey,
    item.assignee ? props.who(`bot:${item.assignee}`) : "Unassigned",
    Priority[item.priority],
    item.size === undefined ? "" : Size[item.size],
    item.labels.join(", "),
  ];
  return (
    <p className="drawer-meta">
      {parts.filter(Boolean).join(" · ")}
      {item.blocked ? (
        <span className="drawer-blocked"> · ⛔ Blocked: {item.blocked.reason}</span>
      ) : null}
    </p>
  );
}

/** The guard check for the next forward move, as guidance (§3.6). */
export function NextStepView(props: {
  readonly step: NextStep;
  readonly assignee: string | null;
}): ReactElement | null {
  const { to, unmet } = props.step;
  if (to === undefined) {
    return null;
  }
  return (
    <section className="drawer-next" aria-label="Next step">
      {unmet.length === 0 ? (
        <p>
          Ready to move to {to.name}
          {props.assignee ? ` — waiting on ${props.assignee}` : ""}
        </p>
      ) : (
        <>
          <p>To reach {to.name}:</p>
          <ul>
            {unmet.map((u) => (
              <li key={u.code}>
                <span aria-hidden="true">✗</span> {u.text}
                {u.fix ? <span className="drawer-dim"> {u.fix}</span> : null}
              </li>
            ))}
          </ul>
        </>
      )}
    </section>
  );
}
