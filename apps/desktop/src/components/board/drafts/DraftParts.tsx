// A draft's parts of the item drawer (0.19.0): the changes strip and the
// Review list with an Undo per change, and the footer with Discard, the
// column to finalize into and Finalize; a refusal lists the column's unmet
// rules in place, as a move does.

import { useState } from "react";
import type { FormEvent, ReactElement } from "react";
import { useEscapeKey } from "../../../hooks/useEscapeKey";
import { useReturnFocus } from "../../../hooks/useReturnFocus";
import type {
  BoardColumn,
  DraftRevision,
  Unmet,
} from "../../../protocol/gen/hermes/board/v1/board_pb";
import { ColumnKind } from "../../../protocol/gen/hermes/board/v1/board_pb";
import OverlayShell from "../../overlay/OverlayShell";
import { UnmetList } from "../MovePopovers";
import { changeWords, unseenLine } from "./draftChanges";

export function ChangesStrip(props: {
  readonly unseen: readonly DraftRevision[];
  readonly who: (actor: string) => string;
  readonly onReview: () => void;
}): ReactElement | null {
  if (props.unseen.length === 0) {
    return null;
  }
  return (
    <p className="draft-changes" role="status">
      <span aria-hidden="true">↻ </span>
      {unseenLine(props.unseen, props.who)}
      <button type="button" className="btn btn-small" onClick={props.onReview}>
        Review
      </button>
    </p>
  );
}

export function DraftReview(props: {
  readonly changes: readonly DraftRevision[];
  readonly undone: ReadonlySet<bigint>;
  readonly who: (actor: string) => string;
  readonly onUndo: (revision: DraftRevision) => void;
  readonly onClose: () => void;
}): ReactElement {
  return (
    <section className="draft-review" aria-label="Changes since you looked">
      <header className="draft-review-head">
        <h4>Changes since you looked</h4>
        <button type="button" className="btn btn-small" onClick={props.onClose}>
          Done
        </button>
      </header>
      {props.changes.length === 0 ? <p className="drawer-empty">Nothing new.</p> : null}
      <ol className="draft-review-list">
        {props.changes.map((r) => {
          const words = changeWords(r);
          const undone = props.undone.has(r.id);
          return (
            <li key={String(r.id)} className={undone ? "draft-undone" : ""}>
              <span className="draft-mark" aria-hidden="true">
                {words.mark}
              </span>
              <span className="draft-change">
                <b>{words.field}</b> <span className="drawer-dim">by {props.who(r.actor)}</span>
                <span className="draft-summary">{words.summary}</span>
                {r.field === "description" && words.after !== undefined ? (
                  <span className="draft-diff">
                    <span className="draft-before">
                      <span className="visually-hidden">Before: </span>
                      {words.before || "—"}
                    </span>
                    <span className="draft-after">
                      <span className="visually-hidden">After: </span>
                      {words.after || "—"}
                    </span>
                  </span>
                ) : null}
              </span>
              {undone ? (
                <span className="draft-undone-word">
                  <span aria-hidden="true">↶ </span>Undone
                </span>
              ) : (
                <button
                  type="button"
                  className="btn btn-small"
                  aria-label={`Undo: ${words.field}, ${words.summary}`}
                  onClick={() => props.onUndo(r)}
                >
                  Undo
                </button>
              )}
            </li>
          );
        })}
      </ol>
    </section>
  );
}

/** The column a draft goes live in by default: the board's first open one. */
function firstOpen(columns: readonly BoardColumn[]): BoardColumn | undefined {
  return columns.find((c) => c.kind === ColumnKind.OPEN) ?? columns[0];
}

export function DraftFooter(props: {
  readonly columns: readonly BoardColumn[];
  readonly busy: boolean;
  readonly unmet: readonly Unmet[] | null;
  readonly onFinalize: (column: string) => void;
  readonly onDiscard: () => void;
}): ReactElement {
  const choices = props.columns.filter((c) => c.kind !== ColumnKind.CANCELLED);
  const [column, setColumn] = useState(firstOpen(choices)?.key ?? "");
  const name = choices.find((c) => c.key === column)?.name ?? column;
  return (
    <footer className="drawer-foot draft-foot">
      {props.unmet === null ? null : (
        <div className="draft-refused" role="alert">
          <p>{`Can't finalize into ${name}:`}</p>
          <UnmetList unmet={props.unmet} />
        </div>
      )}
      <div className="draft-foot-row">
        <button
          type="button"
          className="btn btn-small"
          disabled={props.busy}
          onClick={props.onDiscard}
        >
          Discard
        </button>
        <label className="draft-column">
          <span>Column</span>
          <select value={column} disabled={props.busy} onChange={(e) => setColumn(e.target.value)}>
            {choices.map((c) => (
              <option key={c.key} value={c.key}>
                {c.name}
              </option>
            ))}
          </select>
        </label>
        <button
          type="button"
          className="btn btn-small btn-primary"
          disabled={props.busy || column === ""}
          onClick={() => props.onFinalize(column)}
        >
          Finalize
        </button>
      </div>
    </footer>
  );
}

export function DiscardDialog(props: {
  readonly itemId: string;
  readonly onDiscard: (reason: string) => void;
  readonly onCancel: () => void;
}): ReactElement {
  const [reason, setReason] = useState("");
  useEscapeKey(props.onCancel);
  useReturnFocus();
  const title = `Discard the draft ${props.itemId}`;
  const submit = (event: FormEvent): void => {
    event.preventDefault();
    if (reason.trim() !== "") {
      props.onDiscard(reason.trim());
    }
  };
  return (
    <OverlayShell label={title} onClose={props.onCancel}>
      <form className="confirm-dialog" onSubmit={submit}>
        <h2 className="confirm-title">{title}</h2>
        <p className="confirm-body">
          It leaves the drafts tray and keeps its history. The bots discussing it are told.
        </p>
        <label className="field">
          <span className="field-label">Why</span>
          <input type="text" value={reason} autoFocus onChange={(e) => setReason(e.target.value)} />
        </label>
        <div className="confirm-actions">
          <button type="button" className="btn btn-small" onClick={props.onCancel}>
            Keep it
          </button>
          <button
            type="submit"
            className="btn btn-small btn-danger"
            disabled={reason.trim() === ""}
          >
            Discard
          </button>
        </div>
      </form>
    </OverlayShell>
  );
}
