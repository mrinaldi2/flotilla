// Board settings › Columns (0.19.0): a row per column, "+ Column" at the
// end, and removal that asks where a column's items go first.

import { useState } from "react";
import type { FormEvent, ReactElement } from "react";
import { useDialogKeys } from "../../../hooks/useReturnFocus";
import type { Board, BoardColumn } from "../../../protocol/gen/flotilla/board/v1/board_pb";
import { ColumnKind } from "../../../protocol/gen/flotilla/board/v1/board_pb";
import OverlayShell from "../../overlay/OverlayShell";
import ColumnRow from "./ColumnRow";
import type { ColumnChange } from "./columnEdits";
import { KINDS } from "./rules";

export interface ColumnsActions {
  readonly onChange: (column: BoardColumn, change: ColumnChange) => void;
  readonly onAdd: (name: string, kind: ColumnKind) => void;
  readonly onRemove: (column: BoardColumn, moveItemsTo: string | undefined) => void;
}

interface ColumnsSectionProps extends ColumnsActions {
  readonly board: Board;
  readonly columns: readonly BoardColumn[];
  /** Items per column key, for removal. */
  readonly counts: ReadonlyMap<string, number>;
  readonly disabled: boolean;
  /** The column shown expanded first (stories). */
  readonly initiallyExpanded?: string;
}

function AddColumn(props: {
  readonly onAdd: (name: string, kind: ColumnKind) => void;
}): ReactElement {
  const [name, setName] = useState("");
  const [kind, setKind] = useState(ColumnKind.ACTIVE);
  const submit = (event: FormEvent): void => {
    event.preventDefault();
    if (name.trim() !== "") {
      props.onAdd(name.trim(), kind);
      setName("");
    }
  };
  return (
    <form className="column-add" onSubmit={submit} aria-label="Add a column">
      <input
        type="text"
        aria-label="New column name"
        placeholder="+ Column"
        value={name}
        onChange={(e) => setName(e.target.value)}
      />
      <select
        aria-label="New column kind"
        value={kind}
        onChange={(e) => setKind(Number(e.target.value))}
      >
        {KINDS.map((k) => (
          <option key={k.kind} value={k.kind}>
            {`${k.glyph} ${k.word}`}
          </option>
        ))}
      </select>
      <button type="submit" className="btn btn-small" disabled={name.trim() === ""}>
        Add
      </button>
    </form>
  );
}

export function RemoveColumnDialog(props: {
  readonly column: BoardColumn;
  readonly items: number;
  readonly others: readonly BoardColumn[];
  readonly onRemove: (moveItemsTo: string | undefined) => void;
  readonly onCancel: () => void;
}): ReactElement {
  const { column, items } = props;
  const [to, setTo] = useState(props.others[0]?.key ?? "");
  useDialogKeys(props.onCancel);
  const title = `Remove ${column.name}`;
  return (
    <OverlayShell label={title} onClose={props.onCancel}>
      <div className="confirm-dialog">
        <h2 className="confirm-title">{title}</h2>
        {items === 0 ? (
          <p className="confirm-body">It holds no items.</p>
        ) : (
          <>
            <p className="confirm-body">
              {`It holds ${items} ${items === 1 ? "item" : "items"}. Each is moved first, and the move is recorded on it.`}
            </p>
            <label className="field">
              <span className="field-label">Move its items to</span>
              <select value={to} onChange={(e) => setTo(e.target.value)}>
                {props.others.map((other) => (
                  <option key={other.key} value={other.key}>
                    {other.name}
                  </option>
                ))}
              </select>
            </label>
          </>
        )}
        <div className="confirm-actions">
          <button type="button" className="btn btn-small" onClick={props.onCancel}>
            Cancel
          </button>
          <button
            type="button"
            className="btn btn-small btn-danger"
            onClick={() => props.onRemove(items === 0 ? undefined : to)}
          >
            Remove column
          </button>
        </div>
      </div>
    </OverlayShell>
  );
}

export default function ColumnsSection(props: ColumnsSectionProps): ReactElement {
  const { board, columns, disabled } = props;
  const custom = board.flow !== "release_gated";
  const [expanded, setExpanded] = useState<string | null>(props.initiallyExpanded ?? null);
  const [removing, setRemoving] = useState<BoardColumn | null>(null);
  return (
    <section className="settings-section" aria-label="Columns">
      {custom ? null : (
        <p className="settings-note">
          <span aria-hidden="true">⚑</span> Release-gated: this board keeps the columns of the
          release flow. Rename, reorder, hide or limit them; for your own columns and rules, create
          a custom board.
        </p>
      )}
      <ol className="column-rows">
        {columns.map((column) => (
          <ColumnRow
            // By key alone: a rename or a new limit keeps the row, and its focus.
            key={column.key}
            column={column}
            columns={columns}
            custom={custom}
            disabled={disabled}
            expanded={expanded === column.key}
            onExpand={(open) => setExpanded(open ? column.key : null)}
            onChange={(change) => props.onChange(column, change)}
            onRemove={() => setRemoving(column)}
          />
        ))}
      </ol>
      {custom && !disabled ? <AddColumn onAdd={props.onAdd} /> : null}
      {removing === null ? null : (
        <RemoveColumnDialog
          column={removing}
          items={props.counts.get(removing.key) ?? 0}
          others={columns.filter((c) => c.key !== removing.key)}
          onRemove={(to) => {
            props.onRemove(removing, to);
            setRemoving(null);
          }}
          onCancel={() => setRemoving(null)}
        />
      )}
    </section>
  );
}
