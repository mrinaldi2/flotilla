// A column's On entry role in Board settings (0.24.0, `board_flow_v1`): the
// role a card entering the column goes to, and who holds it; under the
// rows, the roles nobody holds with Create a bot, and the chain a card
// follows. Any board takes it, a release-gated one too.

import type { ReactElement } from "react";
import type { BoardColumn } from "../../../protocol/gen/flotilla/board/v1/board_pb";
import { Role } from "../../../protocol/gen/flotilla/board/v1/board_pb";
import { assignableRoles, roleLabel } from "../../bot/roleLabels";
import type { ColumnChange } from "./columnEdits";
import type { ColumnFlow } from "./columnFlow";
import { flowSteps, heldBy, namesText, sendBackLine, unheldRoles } from "./columnFlow";

/** "On entry, assign [Architect]  Held by Architect · asleep". */
export function OnEntryLine(props: {
  readonly column: BoardColumn;
  readonly flow: ColumnFlow;
  readonly disabled: boolean;
  readonly onChange: (change: ColumnChange) => void;
}): ReactElement {
  const { column } = props;
  const held = heldBy(column.onEntry, props.flow);
  const pick = (value: string): void => {
    const role = Number(value);
    if (Number.isInteger(role) && role !== column.onEntry) {
      props.onChange({ onEntry: role });
    }
  };
  return (
    <div className="column-flow">
      <label className="column-flow-pick">
        <span>On entry, assign</span>
        <select
          aria-label={`On entry, ${column.name} assigns`}
          value={column.onEntry}
          disabled={props.disabled}
          onChange={(e) => pick(e.target.value)}
        >
          <option value={Role.UNSPECIFIED}>Nobody</option>
          {assignableRoles().map(([role, label]) => (
            <option key={role} value={role}>
              {label}
            </option>
          ))}
        </select>
      </label>
      {column.onEntry === Role.UNSPECIFIED ? null : (
        <span
          className={`column-flow-held${held.unheld ? " column-flow-unheld" : ""}`}
          title={`Held by ${held.text}`}
        >
          <span className="column-flow-held-label">Held by</span> {held.text}
        </span>
      )}
    </div>
  );
}

/** "▲ Nobody holds Product — Cards entering To do stay with whoever moved them." */
function UnheldNotices(props: {
  readonly columns: readonly BoardColumn[];
  readonly flow: ColumnFlow;
  readonly disabled: boolean;
}): ReactElement | null {
  const { flow } = props;
  const unheld = unheldRoles(props.columns, flow.roles);
  if (unheld.length === 0) {
    return null;
  }
  return (
    <ul className="column-flow-notices">
      {unheld.map(({ role, columns }) => {
        const label = roleLabel(role);
        const create = flow.onCreateBot;
        return (
          <li key={role} className="column-flow-notice">
            <span className="column-flow-notice-title">
              <span aria-hidden="true">▲ </span>Nobody holds {label}
            </span>
            <span className="column-flow-notice-text">
              {`Cards entering ${namesText(columns)} stay with whoever moved them.`}
            </span>
            {create === undefined || props.disabled ? null : (
              <button
                type="button"
                className="btn btn-small btn-primary"
                disabled={(flow.creating ?? null) !== null}
                onClick={() => create(role)}
              >
                {flow.creating === role ? "Creating…" : `Create a ${label} bot`}
              </button>
            )}
          </li>
        );
      })}
    </ul>
  );
}

/** "To do · Product → Research · Architect → … → Done", and what a failed review does. */
function FlowChain(props: { readonly columns: readonly BoardColumn[] }): ReactElement | null {
  const steps = flowSteps(props.columns);
  if (steps.length === 0) {
    return null;
  }
  const back = sendBackLine(props.columns);
  return (
    <section className="column-flow-chain" aria-label="How a card flows">
      <ol>
        {steps.map((step, at) => (
          <li key={step.key}>
            {at === 0 ? null : <span aria-hidden="true">→ </span>}
            <span className={`column-flow-step${step.done ? " column-flow-step-done" : ""}`}>
              {step.text}
            </span>
          </li>
        ))}
      </ol>
      {back === null ? null : <p className="column-flow-back">{back}</p>}
    </section>
  );
}

/** Under the column rows: the roles nobody holds, then the chain. */
export function FlowSummary(props: {
  readonly columns: readonly BoardColumn[];
  readonly flow: ColumnFlow;
  readonly disabled: boolean;
}): ReactElement {
  return (
    <>
      <UnheldNotices {...props} />
      <FlowChain columns={props.columns} />
    </>
  );
}
