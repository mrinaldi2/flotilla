// One column in Board settings (0.19.0): its place, name, kind chip, WIP
// limit and visibility; expanded, who moves items in, where from, and its
// entry rules. Every save is one board change.

import { ArrowDown, ArrowUp, ChevronDown, ChevronRight, Trash2 } from "lucide-react";
import { useState } from "react";
import type { ReactElement } from "react";
import type { BoardColumn } from "../../../protocol/gen/flotilla/board/v1/board_pb";
import ColumnRules from "./ColumnRules";
import type { ColumnChange } from "./columnEdits";
import { afterFor, limitChange, policyLine } from "./columnEdits";
import { KINDS, kindChip } from "./rules";

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

function OrderButtons(props: {
  readonly column: BoardColumn;
  readonly columns: readonly BoardColumn[];
  readonly disabled: boolean;
  readonly onChange: (change: ColumnChange) => void;
}): ReactElement {
  const { column, disabled } = props;
  const up = afterFor(props.columns, column, -1);
  const down = afterFor(props.columns, column, 1);
  return (
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
  );
}

/**
 * A text field that saves on Enter or when focus leaves it, once per text:
 * Enter then Tab sends one change, not a second one with the version the
 * first has used. It takes the board's value whenever that changes.
 */
function SavedInput(props: {
  readonly label: string;
  readonly className?: string;
  readonly type: "text" | "number";
  readonly initial: string;
  readonly disabled: boolean;
  /** Saves `text`; false puts the field back as it was. */
  readonly onSave: (text: string) => boolean;
}): ReactElement {
  const [text, setText] = useState(props.initial);
  const [shown, setShown] = useState(props.initial);
  const [sent, setSent] = useState<string | null>(null);
  if (props.initial !== shown) {
    setShown(props.initial);
    setText(props.initial);
    setSent(null);
  }
  const save = (): void => {
    if (text === sent) {
      return;
    }
    if (props.onSave(text)) {
      setSent(text);
    } else {
      setText(props.initial);
    }
  };
  return (
    <input
      className={props.className}
      type={props.type}
      min={props.type === "number" ? 1 : undefined}
      placeholder={props.type === "number" ? "–" : undefined}
      aria-label={props.label}
      value={text}
      disabled={props.disabled}
      onChange={(e) => setText(e.target.value)}
      onBlur={save}
      onKeyDown={(e) => e.key === "Enter" && save()}
    />
  );
}

function KindControl(props: {
  readonly column: BoardColumn;
  readonly editable: boolean;
  readonly onChange: (change: ColumnChange) => void;
}): ReactElement {
  const { column } = props;
  if (!props.editable) {
    return <span className="kind-chip">{kindChip(column.kind)}</span>;
  }
  return (
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
  );
}

function RowDetail(props: Omit<ColumnRowProps, "onExpand" | "onRemove">): ReactElement {
  const { column, columns } = props;
  if (!props.expanded) {
    return <p className="column-row-summary">{policyLine(column, columns, props.custom)}</p>;
  }
  if (!props.custom) {
    return (
      <p className="column-rules-hint">
        This board is release-gated: its columns follow the built-in flow of 0.18, so their rules
        can't change. Create a custom board for your own flow.
      </p>
    );
  }
  return (
    <ColumnRules
      column={column}
      others={columns.filter((c) => c.key !== column.key)}
      disabled={props.disabled}
      onSave={(policy) => props.onChange({ policy })}
    />
  );
}

export default function ColumnRow(props: ColumnRowProps): ReactElement {
  const { column, disabled, expanded } = props;
  const editable = props.custom && !disabled;
  const rename = (text: string): boolean => {
    const next = text.trim();
    if (next === "" || next === column.name) {
      return false;
    }
    props.onChange({ name: next });
    return true;
  };
  const setLimit = (text: string): boolean => {
    const change = limitChange(text, column.wipLimit);
    if (change !== null) {
      props.onChange(change);
    }
    return change !== null;
  };
  return (
    <li className={`column-row${column.visible ? "" : " column-row-hidden"}`}>
      <div className="column-row-main">
        <OrderButtons {...props} />
        <SavedInput
          className="column-row-name"
          type="text"
          label={`Name of ${column.name}`}
          initial={column.name}
          disabled={disabled}
          onSave={rename}
        />
        <KindControl column={column} editable={editable} onChange={props.onChange} />
        <span className="column-row-wip">
          <span aria-hidden="true">WIP</span>
          <SavedInput
            type="number"
            label={`WIP limit of ${column.name}`}
            initial={column.wipLimit === undefined ? "" : String(column.wipLimit)}
            disabled={disabled}
            onSave={setLimit}
          />
        </span>
        <label className="column-row-visible">
          <input
            type="checkbox"
            aria-label={`Show ${column.name}`}
            checked={column.visible}
            disabled={disabled}
            onChange={(e) => props.onChange({ visible: e.target.checked })}
          />
          <span aria-hidden="true">{column.visible ? "Shown" : "Hidden"}</span>
        </label>
        {editable ? (
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
      <RowDetail {...props} />
    </li>
  );
}
