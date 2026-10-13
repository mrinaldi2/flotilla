// One column in Board settings (0.19.0): its place, name, kind chip, WIP
// limit and visibility; expanded, who moves items in, where from, and its
// entry rules. Every save is one board change.

import { ArrowDown, ArrowUp, ChevronDown, ChevronRight, Trash2 } from "lucide-react";
import { useState } from "react";
import type { ReactElement } from "react";
import type { BoardColumn, ColumnKind } from "../../../protocol/gen/hermes/board/v1/board_pb";
import ColumnRules from "./ColumnRules";
import type { Policy } from "./ColumnRules";
import { KINDS, kindChip, moverWords, ruleWords } from "./rules";

/** A change to one column, as `column_update` takes it. */
export interface ColumnChange {
  readonly name?: string;
  readonly kind?: ColumnKind;
  readonly wipLimit?: number;
  readonly clearWipLimit?: boolean;
  readonly visible?: boolean;
  /** "" moves it first. */
  readonly after?: string;
  readonly policy?: Policy;
}

interface ColumnRowProps {
  readonly column: BoardColumn;
  readonly columns: readonly BoardColumn[];
  /** A custom board: kinds, rules and removal can change. */
  readonly custom: boolean;
  readonly disabled: boolean;
  readonly expanded: boolean;
  readonly onExpand: (open: boolean) => void;
  readonly onChange: (change: ColumnChange) => void;
  readonly onRemove: () => void;
}

/** The `after` that moves `column` one place up or down, or null at the end. */
export function afterFor(
  columns: readonly BoardColumn[],
  column: BoardColumn,
  by: -1 | 1,
): string | null {
  const at = columns.findIndex((c) => c.key === column.key);
  const to = at + by;
  if (at < 0 || to < 0 || to >= columns.length) {
    return null;
  }
  // Up: after the column two above (or first); down: after the one below.
  return by === -1 ? (columns[to - 1]?.key ?? "") : (columns[to]?.key ?? null);
}

export default function ColumnRow(props: ColumnRowProps): ReactElement {
  const { column, columns, custom, disabled, expanded } = props;
  const [name, setName] = useState(column.name);
  const [wip, setWip] = useState(column.wipLimit === undefined ? "" : String(column.wipLimit));
  const up = afterFor(columns, column, -1);
  const down = afterFor(columns, column, 1);
  const rename = (): void => {
    const next = name.trim();
    if (next !== "" && next !== column.name) {
      props.onChange({ name: next });
    } else {
      setName(column.name);
    }
  };
  const setLimit = (): void => {
    const limit = Number.parseInt(wip, 10);
    if (wip.trim() === "" && column.wipLimit !== undefined) {
      props.onChange({ clearWipLimit: true });
    } else if (Number.isInteger(limit) && limit > 0 && limit !== column.wipLimit) {
      props.onChange({ wipLimit: limit });
    } else {
      setWip(column.wipLimit === undefined ? "" : String(column.wipLimit));
    }
  };
  const summary = !custom
    ? ["Built-in release-gated rules"]
    : [
        moverWords(column.movers),
        ...column.rules.map(ruleWords),
        column.allowedFrom.length === 0
          ? ""
          : `from ${column.allowedFrom.map((k) => columns.find((c) => c.key === k)?.name ?? k).join(", ")}`,
      ].filter(Boolean);

  return (
    <li className={`column-row${column.visible ? "" : " column-row-hidden"}`}>
      <div className="column-row-main">
        <span className="column-row-order">
          <button
            type="button"
            className="icon-btn"
            aria-label={`Move ${column.name} left`}
            disabled={disabled || up === null}
            onClick={() => up !== null && props.onChange({ after: up })}
          >
            <ArrowUp size={13} aria-hidden="true" />
          </button>
          <button
            type="button"
            className="icon-btn"
            aria-label={`Move ${column.name} right`}
            disabled={disabled || down === null}
            onClick={() => down !== null && props.onChange({ after: down })}
          >
            <ArrowDown size={13} aria-hidden="true" />
          </button>
        </span>
        <input
          className="column-row-name"
          type="text"
          aria-label={`Name of ${column.name}`}
          value={name}
          disabled={disabled}
          onChange={(e) => setName(e.target.value)}
          onBlur={rename}
          onKeyDown={(e) => e.key === "Enter" && rename()}
        />
        {custom && !disabled ? (
          <select
            className="column-row-kind"
            aria-label={`Kind of ${column.name}`}
            value={column.kind}
            onChange={(e) => props.onChange({ kind: Number(e.target.value) })}
          >
            {KINDS.map((k) => (
              <option key={k.kind} value={k.kind}>
                {`${k.glyph} ${k.word}`}
              </option>
            ))}
          </select>
        ) : (
          <span className="kind-chip">{kindChip(column.kind)}</span>
        )}
        <label className="column-row-wip">
          <span>WIP</span>
          <input
            type="number"
            min={1}
            aria-label={`WIP limit of ${column.name}`}
            placeholder="–"
            value={wip}
            disabled={disabled}
            onChange={(e) => setWip(e.target.value)}
            onBlur={setLimit}
            onKeyDown={(e) => e.key === "Enter" && setLimit()}
          />
        </label>
        <label className="column-row-visible">
          <input
            type="checkbox"
            checked={column.visible}
            disabled={disabled}
            onChange={(e) => props.onChange({ visible: e.target.checked })}
          />
          <span>{column.visible ? "Shown" : "Hidden"}</span>
        </label>
        {custom && !disabled ? (
          <button
            type="button"
            className="icon-btn"
            aria-label={`Remove ${column.name}`}
            onClick={props.onRemove}
          >
            <Trash2 size={13} aria-hidden="true" />
          </button>
        ) : null}
        <button
          type="button"
          className="icon-btn"
          aria-expanded={expanded}
          aria-label={`${expanded ? "Hide" : "Show"} the rules of ${column.name}`}
          onClick={() => props.onExpand(!expanded)}
        >
          {expanded ? (
            <ChevronDown size={13} aria-hidden="true" />
          ) : (
            <ChevronRight size={13} aria-hidden="true" />
          )}
        </button>
      </div>
      {expanded ? (
        custom ? (
          <ColumnRules
            column={column}
            others={columns.filter((c) => c.key !== column.key)}
            disabled={disabled}
            onSave={(policy) => props.onChange({ policy })}
          />
        ) : (
          <p className="column-rules-hint">
            This board is release-gated: its columns follow the built-in flow of 0.18, so their
            rules can't change. Create a custom board for your own flow.
          </p>
        )
      ) : (
        <p className="column-row-summary">{summary.join(" · ")}</p>
      )}
    </li>
  );
}
