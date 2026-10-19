// One Needs you row (H-018 §2.1): a glyph, what waits and why, and its one
// action, or the computer to act on it off the board's home. A row answered
// in place (0.22.0) opens its answer under it.

import { useId, useState } from "react";
import type { MouseEvent, ReactElement } from "react";
import LinkedText from "../cards/LinkedText";

export interface RowViewProps {
  readonly glyph: string;
  readonly tone?: "bad" | "you";
  readonly title: ReactElement | string;
  readonly meta: string;
  /** None for a row that only informs: its fix is outside the app. */
  readonly action?: string;
  /** Starts with `action`, then says what it acts on (UX-010). */
  readonly label?: string;
  readonly onAction?: (event: MouseEvent<HTMLButtonElement>) => void;
  readonly disabled?: boolean;
  /** Why the action is unavailable, as its tooltip. */
  readonly why?: string;
  /** What to do on the computer that holds it: "Review", "Answer", "Confirm". */
  readonly verb: string;
  /** Off-home: the computer to act on it, in place of the button. */
  readonly elsewhere?: string;
  /** Answered in place (0.22.0): the action opens this under the row, and closes it. */
  readonly inline?: () => ReactElement;
}

/** The row's one button; a row answered in place opens and closes its answer. */
function RowAction(props: {
  readonly row: RowViewProps;
  readonly open: boolean;
  readonly onToggle: () => void;
}): ReactElement | null {
  const { row, open } = props;
  if (row.action === undefined) {
    return null;
  }
  const inline = row.inline !== undefined;
  return (
    <button
      type="button"
      className="btn btn-small"
      aria-label={open ? "Close the question" : row.label}
      title={row.why}
      disabled={row.disabled}
      aria-expanded={inline ? open : undefined}
      onClick={inline ? props.onToggle : row.onAction}
    >
      {open ? "Close" : row.action}
    </button>
  );
}

export default function RowView(props: RowViewProps): ReactElement {
  const there = useId();
  const [open, setOpen] = useState(false);
  const away = props.elsewhere !== undefined;
  const inline = away ? undefined : props.inline;
  return (
    <li
      className={`dash-row${open ? " dash-row-open" : ""}`}
      aria-describedby={away ? there : undefined}
    >
      <span className={`dash-glyph dash-tone-${props.tone ?? "plain"}`} aria-hidden="true">
        {props.glyph}
      </span>
      <div className="dash-row-text">
        <div className="dash-row-title">
          {typeof props.title === "string" ? <LinkedText text={props.title} /> : props.title}
        </div>
        <div className="dash-row-meta">
          <LinkedText text={props.meta} />
        </div>
      </div>
      {away ? (
        <span id={there} className="dash-elsewhere">
          {props.verb} on {props.elsewhere}
        </span>
      ) : (
        <RowAction row={props} open={open} onToggle={() => setOpen((was) => !was)} />
      )}
      {open && inline !== undefined ? <div className="dash-row-inline">{inline()}</div> : null}
    </li>
  );
}
